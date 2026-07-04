//! `view:"…"` cell validation on save (widget-catalog scope, Slice A — the save-gate half). The
//! sibling of [`genui`](super::genui): where `check_genui_cells` validates a `view:"genui"` cell's IR,
//! `check_view_cells` validates every cell's **view NAME** against the embedded widget catalog. The
//! host is the boundary: a hallucinated view (`"heatmap"`, a typo) is REJECTED loudly at write time —
//! for the shell, a `POST /mcp/call` writer, a routed-Zenoh writer, and a headless external agent
//! alike — not degraded to a broken tile at render time (the reported G4 symptom).
//!
//! **Scope is view-NAME only** (widget-catalog scope, Non-goals). A known built-in view (from
//! `widget_catalog.json` — the SAME file `dashboard.catalog` serves, so the verb and the validator
//! agree by construction) is accepted; a well-formed `ext:<id>/<widget>` federation key is accepted
//! **structurally** (not resolved against installs — resolving would couple `dashboard.save` to the
//! extension-install lifecycle and force it to take `&Node`); `genui` defers to `check_genui_cells`;
//! anything else is a loud `BadInput` naming the cell index and the bad view. Option KEYS are NOT
//! validated in this slice (a named follow-up). Store-only, no `Node` — `dashboard.save`'s signature
//! is unchanged.
//!
//! **Why name the cell + view in the error:** `dashboard.save` validates the whole `cells[]`, so ONE
//! unknown-view cell blocks the entire save (even a title edit) — the genui precedent's behavior. The
//! message must make the fix one edit away: `cell {i}: unknown view '{view}' — call dashboard.catalog`.
//!
//! The catalog JSON is the SOURCE OF TRUTH; the `WidgetView` render switch must stay in step with it
//! (widget-catalog scope, "Catalog ↔ renderer drift"). A ui-side consistency test guards that; here
//! the built-in view-name set is read from the same file the verb serves.

use std::collections::HashSet;
use std::sync::OnceLock;

use serde_json::Value;

use super::error::DashboardError;
use super::model::Cell;

/// The hand-authored widget catalog — the built-in view palette (widget-catalog scope). `include_str!`
/// of the checked-in artifact, the same "embed a host-owned catalog" pattern as `genui_catalog.json`.
/// This is the SAME file `dashboard.catalog` serves, so the verb and this validator can never disagree
/// about which built-in views exist.
const CATALOG_JSON: &str = include_str!("widget_catalog.json");

/// The set of built-in view ids the node accepts on save — the palette's `views[].id`. Both
/// `buildable:true` (an AI author may create them) and `buildable:false` (aliases/escape hatches like
/// `chart`/`plot`/`button`, valid to save/render but not to author new) are accepted here: the
/// validator's job is "is this a KNOWN view", not "may a new cell be authored as it".
fn builtin_views() -> &'static HashSet<String> {
    static VIEWS: OnceLock<HashSet<String>> = OnceLock::new();
    VIEWS.get_or_init(|| {
        let doc: Value =
            serde_json::from_str(CATALOG_JSON).expect("widget_catalog.json is valid JSON");
        doc.get("views")
            .and_then(Value::as_array)
            .map(|views| {
                views
                    .iter()
                    .filter_map(|v| v.get("id").and_then(Value::as_str))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    })
}

/// The built-in view ids this node recognises — the same set `check_view_cells` accepts, exposed so a
/// unit test can assert the verb and the validator key off one file (widget-catalog scope, "the verb
/// and the validator agree"). Sorted for a stable comparison.
pub fn builtin_view_ids() -> Vec<String> {
    let mut ids: Vec<String> = builtin_views().iter().cloned().collect();
    ids.sort();
    ids
}

/// Validate every cell's `view` NAME on a save. A known built-in view, a well-formed
/// `ext:<id>/<widget>` key, or `genui` passes; anything else is a loud `BadInput` naming the cell and
/// the offending view. `genui`'s IR is NOT checked here — [`super::genui::check_genui_cells`] owns that
/// (this slice validates the view-name half only). Returns the first failure.
pub fn check_view_cells(cells: &[Cell]) -> Result<(), DashboardError> {
    let views = builtin_views();
    for cell in cells {
        let view = cell.view.as_str();
        // A pre-view (v1) cell may carry no `view` and fall back to `widget_type` at render; an empty
        // view is not a hallucinated view, so we do not reject it here (the render path handles a v1
        // cell). Only a NON-empty, unknown view is a rejection.
        if view.is_empty() {
            continue;
        }
        if view == "genui" {
            continue; // the IR is validated by `check_genui_cells`.
        }
        if views.contains(view) {
            continue;
        }
        if let Some(rest) = view.strip_prefix("ext:") {
            if is_well_formed_ext_key(rest) {
                continue;
            }
            return Err(bad(
                &cell.i,
                format!(
                    "malformed extension view '{view}' — expected 'ext:<id>/<widget>' (both non-empty)"
                ),
            ));
        }
        return Err(bad(
            &cell.i,
            format!("unknown view '{view}' — call dashboard.catalog for the palette"),
        ));
    }
    Ok(())
}

/// Is `rest` (the part after `ext:`) a well-formed `<id>/<widget>` — exactly one `/`, both sides
/// non-empty? Structural only (widget-catalog scope, "Why `ext:` keys are validated structurally"):
/// we do NOT resolve `<id>` against the workspace's installs, so uninstalling an extension never makes
/// a dashboard that mentions its tile unsavable, and `dashboard.save` need not take `&Node`.
fn is_well_formed_ext_key(rest: &str) -> bool {
    match rest.split_once('/') {
        Some((id, widget)) => !id.is_empty() && !widget.is_empty() && !widget.contains('/'),
        None => false,
    }
}

fn bad(cell_i: &str, msg: impl std::fmt::Display) -> DashboardError {
    DashboardError::BadInput(format!("cell {cell_i}: {msg}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The embedded catalog parses, view ids are unique, and every viz view carries a non-empty
    /// options list (widget-catalog scope, "Catalog completeness"). The validator's accept-set is
    /// exactly `views[].id` from this same file — the verb and the validator agree by construction.
    #[test]
    fn catalog_parses_ids_unique_viz_has_options() {
        let doc: Value = serde_json::from_str(CATALOG_JSON).expect("catalog is valid JSON");
        let views = doc["views"].as_array().expect("views is an array");
        let mut seen = HashSet::new();
        for v in views {
            let id = v["id"].as_str().expect("view has a string id");
            assert!(seen.insert(id.to_string()), "duplicate view id: {id}");
            if v["kind"] == Value::from("viz") {
                let opts = v["options"].as_array().expect("viz view has options array");
                assert!(!opts.is_empty(), "viz view {id} has an empty options list");
            }
        }
        // The accept-set is derived from the same file — never a hand-kept second list.
        assert_eq!(builtin_view_ids().len(), seen.len());
        for id in &seen {
            assert!(builtin_views().contains(id), "accept-set missing {id}");
        }
    }
}
