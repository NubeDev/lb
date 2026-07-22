//! `template-group` expansion — the reusable-pages fan-out, server-side, next to the `tag-group`
//! expansion (reusable-pages scope, "Fan-out lives in `nav.resolve`"). ONE authored entry expands to
//! one page **instance per option value** of a template's parameter: `Sites ▸ Plant-1 · Plant-2 · …`,
//! each a link to the SAME dashboard record with `?var-<var>=<value>`. Tag a new site → a new page
//! appears; no nav edit, no dashboard copy (instance = binding, never copy).
//!
//! **The lens, again.** Expansion runs under the CALLER's caps and grants nothing:
//!   - the option source (tag `facets` via `tags.find`'s cap, or a `{tool,args}` query re-entering the
//!     generic dispatcher under the caller's caps) — a caller lacking the source's cap gets the WHOLE
//!     entry stripped, no option value leaked (opaque, mandatory deny test);
//!   - the template dashboard itself must pass the three-gate read (`dashboard.get`) or the entry is
//!     stripped — the caller cannot see a page they cannot read;
//!   - each emitted link is data; the dashboard + every cell source re-check server-side on visit.
//!
//! Distinct from `tag-group` (many dashboards, one entry each): a `template-group` is ONE dashboard,
//! many bindings. The two mechanisms stay orthogonal (reusable-pages scope, rejected alternatives).

use std::collections::BTreeMap;
use std::sync::Arc;

use lb_auth::Principal;
use serde_json::Value;

use super::error::NavError;
use super::model::{NavItem, ResolvedItem, MAX_TAG_GROUP};
use crate::boot::Node;
use crate::dashboard::dashboard_get;
use crate::tags::tags_facet_values;
use crate::tool_call::call_tool_at_depth;

/// Expand a `template-group` item to a `group` of per-value `dashboard` instances, or `None` if the
/// caller can't reach it (the strip). `depth` threads the caller's re-entrancy depth for the query
/// option source (so its target tool re-checks under the caller's caps, no render-path bypass).
pub async fn resolve_template_group(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
    depth: u32,
) -> Result<Option<ResolvedItem>, NavError> {
    // A template-group with no template/parameter is malformed (bounds reject it at save; guard here).
    let dash_id = item
        .dashboard
        .strip_prefix("dashboard:")
        .unwrap_or(&item.dashboard);
    if dash_id.is_empty() || item.var.is_empty() {
        return Ok(None);
    }

    // Gate 1 — the template page must be readable by the caller (three-gate `dashboard.get`). If not,
    // strip the whole entry: no page, no instances (the caller can't see a page they can't read).
    let template = match dashboard_get(&node.store, principal, ws, dash_id).await {
        Ok(d) => d,
        Err(_) => return Ok(None),
    };

    // Enumerate the option VALUES under the caller's caps. A denial (missing tags.find cap, or the
    // query tool's cap) strips the whole entry — no value leaks (the lens, opaque).
    let values = match enumerate_values(node, principal, ws, item, depth).await {
        Ok(vs) => vs,
        Err(_) => return Ok(None),
    };

    // One child link per value (capped like tag-group). label = the value; href carries the binding.
    let mut children = Vec::new();
    for value in values {
        if children.len() >= MAX_TAG_GROUP {
            break; // loud truncation is the tag-group rule; the resolver logs at the call site.
        }
        let mut vars = BTreeMap::new();
        vars.insert(item.var.clone(), value.clone());
        children.push(ResolvedItem {
            kind: "dashboard".into(),
            label: value,
            icon: String::new(),
            surface: String::new(),
            dashboard: format!("dashboard:{dash_id}"),
            ext: String::new(),
            items: Vec::new(),
            vars,
        });
    }

    Ok(Some(ResolvedItem {
        kind: "group".into(),
        label: label_or(&item.label, &template.title),
        icon: item.icon.clone(),
        surface: String::new(),
        dashboard: String::new(),
        ext: String::new(),
        items: children,
        vars: BTreeMap::new(),
    }))
}

/// The option values, from exactly one source: tag `facets` (the common case) or a `{tool,args}`
/// query (the general case). Bounds guaranteed exactly one is set; prefer `facets` if both slip through.
async fn enumerate_values(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    item: &NavItem,
    depth: u32,
) -> Result<Vec<String>, NavError> {
    if let Some(facet) = item.facets.first() {
        // Tag-facet source: the distinct values present for the facet key (gated on `tags.find`).
        let raw = tags_facet_values(&node.store, principal, ws, &facet.key)
            .await
            .map_err(|_| NavError::Denied)?;
        return Ok(dedupe(raw.iter().filter_map(value_to_string)));
    }
    if !item.tool.is_empty() {
        // Query source: re-enter the generic dispatcher under the caller's caps (no bypass). A denial
        // propagates up as `Denied` → the entry strips. Rows → option values (the variable-bar shape).
        let input = item.args.to_string();
        // Box the recursive future: this re-enters the dispatcher, which can route back through
        // `nav.resolve` — a static async cycle Rust requires boxing to size (the viz precedent).
        let out = Box::pin(call_tool_at_depth(
            node,
            principal,
            ws,
            &item.tool,
            &input,
            depth + 1,
        ))
        .await
        .map_err(|_| NavError::Denied)?;
        let parsed: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
        return Ok(dedupe(rows_to_values(&parsed).into_iter()));
    }
    Ok(Vec::new())
}

/// Reduce a tool result to option value strings — the `{rows:[...]}` / bare-array / `{columns,rows}`
/// shapes our read tools return, mirroring the UI `rowsToOptions`. A row is a scalar as-is, else the
/// first of `value`/`name`/`label`/`id`, else its first column.
fn rows_to_values(result: &Value) -> Vec<String> {
    let rows = match result {
        Value::Array(a) => a.clone(),
        Value::Object(o) => match o.get("rows") {
            Some(Value::Array(a)) => a.clone(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    rows.iter().filter_map(row_to_value).collect()
}

fn row_to_value(row: &Value) -> Option<String> {
    match row {
        Value::String(_) | Value::Number(_) | Value::Bool(_) => value_to_string(row),
        Value::Object(o) => {
            for k in ["value", "name", "label", "id"] {
                if let Some(v) = o.get(k) {
                    if let Some(s) = value_to_string(v) {
                        return Some(s);
                    }
                }
            }
            // Fall back to the first column's value.
            o.values().find_map(value_to_string)
        }
        Value::Array(a) => a.first().and_then(value_to_string),
        _ => None,
    }
}

/// A scalar JSON value → its string form (a string as-is, a number/bool stringified). Objects/arrays
/// and null are not a value (`None`), so a weird typed facet value degrades sanely rather than
/// breaking the link grammar (reusable-pages scope, "label falls back sanely for typed values").
fn value_to_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Dedupe preserving first-seen order, dropping empties.
fn dedupe(it: impl Iterator<Item = String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for s in it {
        if s.is_empty() || !seen.insert(s.clone()) {
            continue;
        }
        out.push(s);
    }
    out
}

/// The author label, or a fallback (the template's title) when the author left it empty.
fn label_or(label: &str, fallback: &str) -> String {
    if label.is_empty() {
        fallback.to_string()
    } else {
        label.to_string()
    }
}
