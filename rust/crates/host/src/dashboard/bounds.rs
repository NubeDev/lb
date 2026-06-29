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

/// Reject a cell whose v3 record would exceed the panel-model caps. Bounded growth keeps the
/// dashboard record small for roster/list reads.
pub fn check_cell_bounds(cell: &Cell) -> Result<(), DashboardError> {
    if cell.transformations.len() > MAX_TRANSFORMS {
        return Err(DashboardError::BadInput(format!(
            "cell {}: {} transformations exceeds cap {MAX_TRANSFORMS}",
            cell.i,
            cell.transformations.len()
        )));
    }
    check_field_options(&field_config_defaults(&cell.field_config), &cell.i)?;
    for over in field_config_overrides(&cell.field_config) {
        // An override carries `properties[]`; the field-option caps apply to the properties it sets,
        // counted leniently via the same mapping/threshold inspection on its `properties` values.
        check_override(over, &cell.i)?;
    }
    let n_over = field_config_overrides(&cell.field_config).len();
    if n_over > MAX_OVERRIDES {
        return Err(DashboardError::BadInput(format!(
            "cell {}: {n_over} fieldConfig overrides exceeds cap {MAX_OVERRIDES}",
            cell.i
        )));
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
fn check_field_options(opts: &Value, cell_i: &str) -> Result<(), DashboardError> {
    if let Some(Value::Array(m)) = opts.get("mappings") {
        if m.len() > MAX_MAPPINGS {
            return Err(DashboardError::BadInput(format!(
                "cell {cell_i}: {} mappings exceeds cap {MAX_MAPPINGS}",
                m.len()
            )));
        }
    }
    if let Some(Value::Array(s)) = opts.get("thresholds").and_then(|t| t.get("steps")) {
        if s.len() > MAX_THRESHOLD_STEPS {
            return Err(DashboardError::BadInput(format!(
                "cell {cell_i}: {} threshold steps exceeds cap {MAX_THRESHOLD_STEPS}",
                s.len()
            )));
        }
    }
    Ok(())
}

/// Bound a single override's property values (mappings/thresholds carried via `properties[].value`).
fn check_override(over: &Value, cell_i: &str) -> Result<(), DashboardError> {
    if let Some(Value::Array(props)) = over.get("properties") {
        for p in props {
            if let Some(v) = p.get("value") {
                check_field_options(&json_wrap_property(p, v), cell_i)?;
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
