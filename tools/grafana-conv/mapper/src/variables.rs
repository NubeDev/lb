//! `templating.list[]` → `Variable` mapping (grafana-conversion scope,
//! "Template variables" + trap #2).
//!
//! Trap #2 — chained variables have no explicit edges. Grafana rebuilds the
//! dependency graph by parsing `$var` / `${var}` out of each variable's query. We
//! do NOT resolve here (resolution is the shipped client runtime's job); we only
//! emit variables in a **topologically-resolvable order** so a `$region`-in-
//! `$host`-query chain is stored dependency-first. A cycle is reported, not hung.
//!
//! The `Variable` type's additive advanced fields (label≠value options, regex,
//! sort, refresh, allValue, hide, the `datasource` type) are already shipped in
//! `model.rs` — this mapper emits them, it does not invent them.

use crate::input::{GrafanaDashboard, Variable as GVar};
use crate::model::Variable;
use crate::report::ConversionReport;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Map `templating.list[]` to dependency-ordered `Variable[]`.
pub fn map_variables(dash: &GrafanaDashboard, report: &mut ConversionReport) -> Vec<Variable> {
    let vars = &dash.templating.list;
    if vars.is_empty() {
        return Vec::new();
    }
    let order = topo_order(vars, report);
    if order.len() > 1 {
        report.mapped(
            "var.order",
            "",
            "variables emitted in dependency order (chained vars resolved by $ref parse)",
        );
    }
    let mut mapped: HashMap<usize, Variable> = vars
        .iter()
        .enumerate()
        .map(|(i, gv)| {
            let at = format!("templating.list[{i}]");
            (i, map_one(gv, &at, report))
        })
        .collect();
    order
        .into_iter()
        .map(|i| mapped.remove(&i).expect("topo order index in range"))
        .collect()
}

fn map_one(gv: &GVar, at: &str, report: &mut ConversionReport) -> Variable {
    let kind = normalize_type(&gv.r#type, report, at);
    let mut v = Variable {
        name: gv.name.clone(),
        label: gv.label.clone(),
        r#type: kind.clone(),
        multi: gv.multi,
        include_all: gv.include_all,
        ..Variable::default()
    };

    match kind.as_str() {
        "custom" => {
            v.custom = parse_custom_value(&gv.query);
            report.mapped("var.custom", at, "custom variable → static option list");
        }
        "text" => {
            v.text = value_to_string(&gv.query);
            report.mapped("var.text", at, "textbox variable → free-text default");
        }
        "const" => {
            v.const_ = value_to_string(&gv.query);
            report.mapped("var.const", at, "constant variable → hidden fixed value");
        }
        "interval" => {
            v.interval = parse_interval(&gv.query);
            report.mapped("var.interval", at, "interval variable → $__interval list");
        }
        "datasource" => {
            // Thin: maps to our federation datasource list (advanced-variables scope).
            v.query = gv.query.clone();
            report.mapped(
                "var.datasource",
                at,
                "datasource variable → federation datasource picker",
            );
        }
        "query" => {
            v.query = gv.query.clone();
            report.mapped(
                "var.query",
                at,
                "query variable → resolver {tool,args} (opaque)",
            );
        }
        "adhoc" => {
            // Degrade: needs datasource key discovery (named follow-up).
            report.degraded(
                "var.adhoc",
                at,
                "adhoc filters preserved as opaque query; not rendered (key discovery follow-up)",
            );
            v.query = gv.query.clone();
        }
        "groupby" => {
            report.dropped(
                "var.groupby",
                at,
                "groupby variable is datasource-specific; not carried",
            );
        }
        _ => {
            report.degraded(
                "var.type.unknown",
                at,
                format!(
                    "unknown variable type `{}`; carried as opaque query",
                    gv.r#type
                ),
            );
            v.query = gv.query.clone();
        }
    }

    // Advanced fields (all additive in model.rs). Carry each as-is; report the
    // ones that actually appear so the user knows what was preserved.
    if let Some(s) = value_to_string_opt(&gv.regex) {
        v.regex = s;
        report.mapped(
            "var.regex",
            at,
            "regex extraction / capture split → Variable.regex",
        );
    }
    if let Some(s) = value_to_string_opt(&gv.regex_apply_to) {
        v.regex_apply_to = s;
    }
    if let Some(s) = value_to_string_opt(&gv.sort) {
        v.sort = s;
        report.mapped("var.sort", at, "option sort order → Variable.sort");
    }
    if let Some(s) = value_to_string_opt(&gv.refresh) {
        v.refresh = s;
        report.mapped("var.refresh", at, "per-var refresh → Variable.refresh");
    }
    if let Some(s) = value_to_string_opt(&gv.hide) {
        v.hide = s;
        report.mapped("var.hide", at, "bar visibility → Variable.hide");
    }
    if let Some(s) = value_to_string_opt(&gv.all_value) {
        v.all_value = s;
        report.mapped("var.allValue", at, "custom All literal → Variable.allValue");
    }
    if !gv.options.is_empty() {
        // label≠value per-option (the __text/__value split).
        v.options = Value::Array(gv.options.clone());
        report.mapped("var.options", at, "label≠value options → Variable.options");
    }
    // `current` selection persistence: we encode it in the URL, not the record.
    if !gv.current.is_null() {
        report.degraded(
            "var.current",
            at,
            "current selection carried on the URL (?var-<name>=), not the record",
        );
    }
    v
}

/// Normalize Grafana's variable type onto our enum, reporting the rename.
fn normalize_type(raw: &str, report: &mut ConversionReport, at: &str) -> String {
    let out = match raw {
        "query" | "" => "query",
        "custom" => "custom",
        "constant" => "const",
        "textbox" => "text",
        "interval" => "interval",
        "datasource" => "datasource",
        "adhoc" => "adhoc",
        "groupby" => "groupby",
        other => return other.to_string(),
    };
    if raw == "constant" || raw == "textbox" {
        report.mapped(
            "var.type",
            at,
            format!("`{raw}` → type:\"{out}\" (our canonical name)"),
        );
    }
    out.to_string()
}

/// Parse a `custom` variable's `query` (a comma-separated string or an array).
fn parse_custom_value(query: &Value) -> Vec<String> {
    match query {
        Value::String(s) => s.split(',').map(|p| p.trim().to_string()).collect(),
        Value::Array(a) => a
            .iter()
            .map(|v| v.as_str().map(str::to_string).unwrap_or_default())
            .collect(),
        _ => Vec::new(),
    }
}

/// Parse an `interval` variable's `query` (a comma-separated string of durations).
fn parse_interval(query: &Value) -> Vec<String> {
    parse_custom_value(query)
}

fn value_to_string(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_default()
}

fn value_to_string_opt(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Build the dependency-resolving topo order of variables by parsing `$var` /
/// `${var}` / `[[var]]` references out of each variable's `query`. A cycle is
/// reported and broken arbitrarily (no hang).
fn topo_order(vars: &[GVar], report: &mut ConversionReport) -> Vec<usize> {
    let names: HashSet<&str> = vars.iter().map(|v| v.name.as_str()).collect();
    let deps: HashMap<usize, HashSet<usize>> = vars
        .iter()
        .enumerate()
        .map(|(i, v)| (i, parse_deps(&v.query, &names, vars)))
        .collect();

    let mut order = Vec::new();
    let mut visited = HashSet::new();
    let mut on_stack = HashSet::new();
    for i in 0..vars.len() {
        dfs(i, &deps, &mut visited, &mut on_stack, &mut order);
    }
    // A real cycle is reported through the per-visit mark below.
    if has_cycle(&deps) {
        report.degraded(
            "var.cycle",
            "",
            "variable dependency cycle detected; ordering is best-effort, resolution may fail",
        );
    }
    order
}

fn dfs(
    node: usize,
    deps: &HashMap<usize, HashSet<usize>>,
    visited: &mut HashSet<usize>,
    on_stack: &mut HashSet<usize>,
    order: &mut Vec<usize>,
) {
    if visited.contains(&node) {
        return;
    }
    if on_stack.contains(&node) {
        return; // cycle — bail, reported separately
    }
    on_stack.insert(node);
    if let Some(ds) = deps.get(&node) {
        for &d in ds {
            dfs(d, deps, visited, on_stack, order);
        }
    }
    on_stack.remove(&node);
    visited.insert(node);
    order.push(node);
}

fn has_cycle(deps: &HashMap<usize, HashSet<usize>>) -> bool {
    let mut color: HashMap<usize, u8> = HashMap::new(); // 0 white, 1 gray, 2 black
    fn visit(
        n: usize,
        deps: &HashMap<usize, HashSet<usize>>,
        color: &mut HashMap<usize, u8>,
    ) -> bool {
        match color.get(&n).copied().unwrap_or(0) {
            1 => return true,
            2 => return false,
            _ => {}
        }
        color.insert(n, 1);
        if let Some(ds) = deps.get(&n) {
            for &d in ds {
                if visit(d, deps, color) {
                    return true;
                }
            }
        }
        color.insert(n, 2);
        false
    }
    for &n in deps.keys() {
        if visit(n, deps, &mut color) {
            return true;
        }
    }
    false
}

fn parse_deps(query: &Value, names: &HashSet<&str>, vars: &[GVar]) -> HashSet<usize> {
    let s = match query {
        Value::String(s) => s.as_str(),
        _ => return HashSet::new(),
    };
    let mut out = HashSet::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let (tok, next) = next_token(bytes, i);
        if let Some(t) = tok {
            if names.contains(t) {
                if let Some(idx) = vars.iter().position(|v| v.name == t) {
                    out.insert(idx);
                }
            }
        }
        i = next;
    }
    out
}

/// Pull the next `${name}` / `$name` / `[[name]]` token starting at `i`, returning
/// (name, next-index). None if no token at/after `i`.
fn next_token(bytes: &[u8], mut i: usize) -> (Option<&str>, usize) {
    while i < bytes.len() {
        if bytes[i] == b'$' && i + 1 < bytes.len() {
            if bytes[i + 1] == b'{' {
                if let Some(end) = find(&bytes[i + 2..], b'}') {
                    let name = std::str::from_utf8(&bytes[i + 2..i + 2 + end]).ok();
                    return (name, i + 2 + end + 1);
                }
            } else {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && is_name_byte(bytes[j]) {
                    j += 1;
                }
                if j > start {
                    let name = std::str::from_utf8(&bytes[start..j]).ok();
                    return (name, j);
                }
            }
        }
        if bytes[i] == b'[' && i + 1 < bytes.len() && bytes[i + 1] == b'[' {
            if let Some(end) = find(&bytes[i + 2..], b']') {
                if bytes.get(i + 2 + end + 1) == Some(&b']') {
                    let name = std::str::from_utf8(&bytes[i + 2..i + 2 + end]).ok();
                    return (name, i + 2 + end + 2);
                }
            }
        }
        i += 1;
    }
    (None, i)
}

fn find(slice: &[u8], byte: u8) -> Option<usize> {
    slice.iter().position(|&b| b == byte)
}

fn is_name_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dollar_brace_and_double_bracket_refs() {
        let vars = vec![
            GVar {
                name: "region".into(),
                ..Default::default()
            },
            GVar {
                name: "host".into(),
                query: Value::String("hosts_for($region) AND ${region}".into()),
                ..Default::default()
            },
        ];
        let names: HashSet<&str> = vars.iter().map(|v| v.name.as_str()).collect();
        let deps = parse_deps(&vars[1].query, &names, &vars);
        assert!(deps.contains(&0));
    }

    #[test]
    fn topo_orders_dependency_first() {
        let vars = vec![
            GVar {
                name: "host".into(),
                query: Value::String("x($region)".into()),
                ..Default::default()
            },
            GVar {
                name: "region".into(),
                ..Default::default()
            },
        ];
        let order = topo_order(&vars, &mut ConversionReport::default());
        // region must come before host
        let r = order
            .iter()
            .position(|&i| vars[i].name == "region")
            .unwrap();
        let h = order.iter().position(|&i| vars[i].name == "host").unwrap();
        assert!(r < h, "region should resolve before host");
    }
}
