//! `cell→grafana` — map one of our [`Cell`]s back to a Grafana panel (viz import-export scope, the
//! export half of the mapper; the inverse of [`super::to_cell`]). Re-emits the bounded `_grafana`
//! passthrough FIRST, then overlays the mapped typed fields so **mapped fields win over passthrough**
//! (the scope's "passthrough fills only gaps" rule) — a field we now own can't be shadowed by a stale
//! blob. An `unsupported`-placeholder cell exports back to its ORIGINAL Grafana type (recorded in
//! `options.unsupportedType`) so a re-import degrades identically rather than the export lying.

use serde_json::{Map, Value};

use crate::dashboard::model::Cell;

use super::view_alias::panel_type_for_view;

/// Serialize one `Cell` to a Grafana panel object.
pub fn cell_to_panel(cell: &Cell) -> Value {
    // Start from the passthrough (unknown Grafana fields) so mapped fields overlay it.
    let mut panel: Map<String, Value> = match &cell.grafana_passthrough {
        Value::Object(m) => m.clone(),
        _ => Map::new(),
    };

    // `type`: an unsupported placeholder re-emits its original type; else the view's canonical type.
    let panel_type = cell
        .options
        .get("unsupportedType")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| panel_type_for_view(&cell.view).to_string());
    panel.insert("type".to_string(), Value::String(panel_type));

    // numeric id round-trips when the cell key parses as one (import keyed by Grafana id when present).
    if let Ok(id) = cell.i.parse::<u64>() {
        panel.insert("id".to_string(), Value::from(id));
    }
    if !cell.title.is_empty() {
        panel.insert("title".to_string(), Value::String(cell.title.clone()));
    }
    if !cell.description.is_empty() {
        panel.insert(
            "description".to_string(),
            Value::String(cell.description.clone()),
        );
    }
    panel.insert(
        "gridPos".to_string(),
        serde_json::json!({ "x": cell.x, "y": cell.y, "w": cell.w, "h": cell.h }),
    );

    // targets[]: each source's `args` was the original target object; re-emit it, restoring `refId`.
    let targets: Vec<Value> = cell
        .sources
        .iter()
        .map(|src| {
            let mut t = match &src.args {
                Value::Object(m) => m.clone(),
                _ => Map::new(),
            };
            if !src.ref_id.is_empty() {
                t.insert("refId".to_string(), Value::String(src.ref_id.clone()));
            }
            if !src.datasource.is_null() {
                t.insert("datasource".to_string(), src.datasource.clone());
            }
            Value::Object(t)
        })
        .collect();
    if !targets.is_empty() {
        panel.insert("targets".to_string(), Value::Array(targets));
    }

    // Panel-level datasource: the first source's datasource is the panel default (Grafana convention).
    if let Some(first) = cell.sources.first() {
        if !first.datasource.is_null() {
            panel.insert("datasource".to_string(), first.datasource.clone());
        }
    }

    // fieldConfig / transformations / options: mapped fields win.
    if !cell.field_config.is_null() {
        panel.insert("fieldConfig".to_string(), cell.field_config.clone());
    }
    if !cell.transformations.is_empty() {
        panel.insert(
            "transformations".to_string(),
            Value::Array(cell.transformations.clone()),
        );
    }
    // Export `options` without our internal `unsupportedType` marker (it's not a Grafana field).
    let options = strip_internal_options(&cell.options);
    if !options.is_null() {
        panel.insert("options".to_string(), options);
    }
    if !cell.plugin_version.is_empty() {
        panel.insert(
            "pluginVersion".to_string(),
            Value::String(cell.plugin_version.clone()),
        );
    }
    if !cell.links.is_empty() {
        panel.insert("links".to_string(), Value::Array(cell.links.clone()));
    }
    if cell.transparent {
        panel.insert("transparent".to_string(), Value::Bool(true));
    }
    overlay_query_options(&mut panel, cell);

    Value::Object(panel)
}

/// Drop our internal `options.unsupportedType` marker — it's carried in `type` on export, not options.
fn strip_internal_options(options: &Value) -> Value {
    match options {
        Value::Object(m) if m.contains_key("unsupportedType") => {
            let cleaned: Map<String, Value> = m
                .iter()
                .filter(|(k, _)| k.as_str() != "unsupportedType")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if cleaned.is_empty() {
                Value::Null
            } else {
                Value::Object(cleaned)
            }
        }
        other => other.clone(),
    }
}

/// Re-emit the panel-level time override + query options onto the panel (inverse of `to_cell`).
fn overlay_query_options(panel: &mut Map<String, Value>, cell: &Cell) {
    let q = &cell.query_options;
    if q.max_data_points > 0 {
        panel.insert("maxDataPoints".to_string(), Value::from(q.max_data_points));
    }
    if !q.min_interval.is_empty() {
        panel.insert(
            "interval".to_string(),
            Value::String(q.min_interval.clone()),
        );
    }
    if !q.time_from.is_empty() {
        panel.insert("timeFrom".to_string(), Value::String(q.time_from.clone()));
    }
    if !q.time_shift.is_empty() {
        panel.insert("timeShift".to_string(), Value::String(q.time_shift.clone()));
    }
    if q.hide_time_override {
        panel.insert("hideTimeOverride".to_string(), Value::Bool(true));
    }
}

#[cfg(test)]
mod tests {
    use super::super::to_cell::panel_to_cell;
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_a_supported_panel() {
        let orig = json!({
            "id": 4,
            "type": "timeseries",
            "title": "CPU",
            "gridPos": {"x": 0, "y": 0, "w": 12, "h": 8},
            "datasource": {"uid": "prom"},
            "targets": [{"refId": "A", "expr": "up", "datasource": {"uid": "prom"}}],
            "fieldConfig": {"defaults": {"unit": "percent"}},
            "customPluginField": 42
        });
        let cell = panel_to_cell(&orig, 0, &mut vec![]);
        let back = cell_to_panel(&cell);
        assert_eq!(back["type"], json!("timeseries"));
        assert_eq!(back["id"], json!(4));
        assert_eq!(back["title"], json!("CPU"));
        assert_eq!(back["gridPos"], json!({"x": 0, "y": 0, "w": 12, "h": 8}));
        assert_eq!(
            back["fieldConfig"],
            json!({"defaults": {"unit": "percent"}})
        );
        assert_eq!(back["targets"][0]["refId"], json!("A"));
        assert_eq!(back["targets"][0]["expr"], json!("up"));
        // unknown field survived via passthrough
        assert_eq!(back["customPluginField"], json!(42));
    }

    #[test]
    fn unsupported_placeholder_exports_original_type() {
        let orig = json!({"id": 7, "type": "heatmap", "title": "Heat", "heatmapField": 1});
        let cell = panel_to_cell(&orig, 0, &mut vec![]);
        let back = cell_to_panel(&cell);
        // exports the ORIGINAL type, not "json", and drops the internal marker
        assert_eq!(back["type"], json!("heatmap"));
        assert!(back
            .get("options")
            .is_none_or(|o| o.get("unsupportedType").is_none()));
        assert_eq!(back["heatmapField"], json!(1));
    }

    #[test]
    fn mapped_field_wins_over_stale_passthrough() {
        // A passthrough that still carries an old title must NOT shadow the mapped title.
        let mut cell = panel_to_cell(
            &json!({"id": 1, "type": "stat", "title": "New"}),
            0,
            &mut vec![],
        );
        cell.grafana_passthrough = json!({"title": "Stale", "extra": true});
        let back = cell_to_panel(&cell);
        assert_eq!(back["title"], json!("New"));
        assert_eq!(back["extra"], json!(true));
    }
}
