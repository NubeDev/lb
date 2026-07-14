//! Cell v3 record bounds (viz panel-model scope, "Data": bounded so a `fieldConfig`/transform list
//! cannot bloat the dashboard row and slow the roster/list read). The host is the authority — it
//! rejects an over-cap save rather than silently storing it unbounded (the scope's "rejected, not
//! silently stored" rule). The UI mirrors these caps for a friendly error, but this is the boundary.
//!
//! The shapes are opaque `Value` on the host (the UI owns the typed `fieldConfig`/`transformation`),
//! so we count the relevant arrays by JSON shape — `transformations[]`, `fieldConfig.overrides[]`,
//! and, within each field, `mappings[]` / `thresholds.steps[]`.

use serde_json::Value;

use super::error::DashboardError;
use super::model::Cell;

/// Max client-side transformations per panel.
pub const MAX_TRANSFORMS: usize = 32;
/// Max per-field overrides in a panel's `fieldConfig`.
pub const MAX_OVERRIDES: usize = 64;
/// Max value mappings on one field option set.
pub const MAX_MAPPINGS: usize = 64;
/// Max threshold steps on one field option set.
pub const MAX_THRESHOLD_STEPS: usize = 64;
/// Max serialized size of a cell's `_grafana` import/export passthrough blob (viz import-export
/// scope: "bounded `_grafana` passthrough … ≤8 KB/cell" — an oversized blob is rejected, not stored
/// unbounded, so a hostile/huge import cannot bloat the dashboard row).
pub const MAX_GRAFANA_PASSTHROUGH: usize = 8 * 1024;

/// Reject a cell whose v3 record would exceed the panel-model caps. Bounded growth keeps the
/// dashboard record small for roster/list reads. Delegates to [`check_spec_bounds`] — the same
/// non-layout-spec check a library panel reuses (library-panels scope: "same record-growth bounds").
pub fn check_cell_bounds(cell: &Cell) -> Result<(), DashboardError> {
    check_spec_bounds(&cell.transformations, &cell.field_config, &cell.i)
        .map_err(DashboardError::BadInput)?;
    check_passthrough_bounds(&cell.grafana_passthrough, &cell.i).map_err(DashboardError::BadInput)
}

/// Reject a cell whose `_grafana` passthrough blob exceeds [`MAX_GRAFANA_PASSTHROUGH`]. Empty/null
/// (the non-imported case) costs nothing. Measured on the serialized bytes — the same thing stored.
pub fn check_passthrough_bounds(passthrough: &Value, label: &str) -> Result<(), String> {
    if passthrough.is_null() {
        return Ok(());
    }
    let n = serde_json::to_string(passthrough)
        .map(|s| s.len())
        .unwrap_or(0);
    if n > MAX_GRAFANA_PASSTHROUGH {
        return Err(format!(
            "{label}: _grafana passthrough {n} bytes exceeds cap {MAX_GRAFANA_PASSTHROUGH}"
        ));
    }
    Ok(())
}

/// Bound the non-layout spec pieces (`transformations[]` + `fieldConfig`) — the check shared by a
/// dashboard `Cell` and a library `panel` record (both store the SAME v3 spec). `label` names the
/// offending cell/panel in the error. Returns an error **message**; each caller wraps it in its own
/// error type (`DashboardError::BadInput` / `PanelError::BadInput`).
pub fn check_spec_bounds(
    transformations: &[Value],
    field_config: &Value,
    label: &str,
) -> Result<(), String> {
    if transformations.len() > MAX_TRANSFORMS {
        return Err(format!(
            "{label}: {} transformations exceeds cap {MAX_TRANSFORMS}",
            transformations.len()
        ));
    }
    check_field_options(&field_config_defaults(field_config), label)?;
    for over in field_config_overrides(field_config) {
        // An override carries `properties[]`; the field-option caps apply to the properties it sets,
        // counted leniently via the same mapping/threshold inspection on its `properties` values.
        check_override(over, label)?;
    }
    let n_over = field_config_overrides(field_config).len();
    if n_over > MAX_OVERRIDES {
        return Err(format!(
            "{label}: {n_over} fieldConfig overrides exceeds cap {MAX_OVERRIDES}"
        ));
    }
    Ok(())
}

/// All cells in a save, bounded.
pub fn check_cells_bounds(cells: &[Cell]) -> Result<(), DashboardError> {
    for cell in cells {
        check_cell_bounds(cell)?;
    }
    Ok(())
}

fn field_config_defaults(fc: &Value) -> Value {
    fc.get("defaults").cloned().unwrap_or(Value::Null)
}

fn field_config_overrides(fc: &Value) -> &[Value] {
    match fc.get("overrides") {
        Some(Value::Array(a)) => a.as_slice(),
        _ => &[],
    }
}

/// Bound the `mappings[]` / `thresholds.steps[]` of one field-option object.
fn check_field_options(opts: &Value, label: &str) -> Result<(), String> {
    if let Some(Value::Array(m)) = opts.get("mappings") {
        if m.len() > MAX_MAPPINGS {
            return Err(format!(
                "{label}: {} mappings exceeds cap {MAX_MAPPINGS}",
                m.len()
            ));
        }
    }
    if let Some(Value::Array(s)) = opts.get("thresholds").and_then(|t| t.get("steps")) {
        if s.len() > MAX_THRESHOLD_STEPS {
            return Err(format!(
                "{label}: {} threshold steps exceeds cap {MAX_THRESHOLD_STEPS}",
                s.len()
            ));
        }
    }
    Ok(())
}

/// Bound a single override's property values (mappings/thresholds carried via `properties[].value`).
fn check_override(over: &Value, label: &str) -> Result<(), String> {
    if let Some(Value::Array(props)) = over.get("properties") {
        for p in props {
            if let Some(v) = p.get("value") {
                check_field_options(&json_wrap_property(p, v), label)?;
            }
        }
    }
    Ok(())
}

/// A Grafana override property is `{ id: "mappings", value: [...] }` (or `thresholds`). Re-shape it
/// into a `{ mappings | thresholds: value }` object so the same `check_field_options` inspection runs.
fn json_wrap_property(p: &Value, v: &Value) -> Value {
    let id = p.get("id").and_then(Value::as_str).unwrap_or("");
    match id {
        "mappings" => serde_json::json!({ "mappings": v }),
        "thresholds" => serde_json::json!({ "thresholds": v }),
        _ => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_passthrough_costs_nothing() {
        assert!(check_passthrough_bounds(&Value::Null, "c1").is_ok());
        assert!(check_passthrough_bounds(&json!({"a": 1}), "c1").is_ok());
    }

    #[test]
    fn oversized_passthrough_rejected() {
        // A blob well past the 8 KB cap (viz import-export scope: rejected, not stored unbounded).
        let big = json!({ "junk": "x".repeat(MAX_GRAFANA_PASSTHROUGH + 100) });
        let err = check_passthrough_bounds(&big, "c1").unwrap_err();
        assert!(err.contains("exceeds cap"));
        assert!(err.contains("c1"));
    }

    #[test]
    fn cell_bounds_include_passthrough() {
        let mut cell = Cell::default();
        cell.i = "c9".into();
        cell.grafana_passthrough = json!({ "junk": "y".repeat(MAX_GRAFANA_PASSTHROUGH + 1) });
        assert!(check_cell_bounds(&cell).is_err());
    }
}
