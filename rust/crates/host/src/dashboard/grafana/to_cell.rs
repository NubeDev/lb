//! `grafana→cell` — map one migrated Grafana panel onto our [`Cell`] (viz import-export scope, the
//! import half of the mapper). Mechanical, per the scope's mapping table: `gridPos→x/y/w/h`,
//! `type→view` (alias table; unsupported → the honest `json` placeholder + a degraded flag),
//! `targets[]→sources[]`, `fieldConfig`/`transformations`/`options` 1:1, and every field we did NOT
//! consume preserved into the bounded `_grafana` passthrough for a lossless round-trip.
//!
//! The datasource `uid`s inside `sources[]` are the MIGRATED (P3-resolved) refs; the verb's remap step
//! rewrites them to our workspace datasources afterward (`datasources::apply`). This file does not
//! touch tenancy — it only shapes the record.

use serde_json::{Map, Value};

use crate::dashboard::model::{Cell, Target};

use super::view_alias::view_for_panel_type;
use super::DegradedItem;

/// The keys a panel object we consume directly into typed `Cell` fields — everything else falls into
/// the `_grafana` passthrough so export can re-emit it.
const CONSUMED_KEYS: &[&str] = &[
    "type",
    "title",
    "description",
    "gridPos",
    "targets",
    "datasource",
    "fieldConfig",
    "transformations",
    "options",
    "id",
    "pluginVersion",
    "links",
    "transparent",
    "timeFrom",
    "timeShift",
    "hideTimeOverride",
    "maxDataPoints",
    "interval",
    "repeat",
    "repeatDirection",
    "maxPerRow",
];

/// Map one Grafana panel to a `Cell`. `index` seeds the cell key `i` when the panel has no usable id.
/// Pushes a `DegradedItem` for an unsupported panel type (still imported — as the `json` placeholder
/// with the original type recorded in `options.unsupportedType` + the full panel in `_grafana`).
pub fn panel_to_cell(panel: &Value, index: usize, degraded: &mut Vec<DegradedItem>) -> Cell {
    let obj = panel.as_object().cloned().unwrap_or_default();
    let panel_type = obj.get("type").and_then(Value::as_str).unwrap_or("");
    let i = cell_key(&obj, index);

    let (view, mut options, unsupported) = match view_for_panel_type(panel_type) {
        Some(v) => (
            v.to_string(),
            obj.get("options").cloned().unwrap_or(Value::Null),
            false,
        ),
        None => {
            degraded.push(DegradedItem {
                kind: "panel".to_string(),
                cell: i.clone(),
                detail: format!("unsupported panel type '{panel_type}' — rendered as raw json"),
            });
            // Honest placeholder: `json` is a shipped built-in that renders the raw data, and we record
            // the original type so the UI can label it "unsupported: <type>".
            ("json".to_string(), Value::Object(Map::new()), true)
        }
    };
    if unsupported {
        if let Value::Object(m) = &mut options {
            m.insert(
                "unsupportedType".to_string(),
                Value::String(panel_type.to_string()),
            );
        }
    }

    let (x, y, w, h) = grid_pos(&obj);
    let sources = targets_to_sources(&obj);

    Cell {
        i,
        x,
        y,
        w,
        h,
        v: 3,
        view,
        title: obj
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        description: obj
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        sources,
        options,
        transformations: obj
            .get("transformations")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        field_config: obj.get("fieldConfig").cloned().unwrap_or(Value::Null),
        plugin_version: obj
            .get("pluginVersion")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        links: obj
            .get("links")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        transparent: obj
            .get("transparent")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        repeat: obj
            .get("repeat")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        repeat_direction: obj
            .get("repeatDirection")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        max_per_row: obj.get("maxPerRow").and_then(Value::as_u64).unwrap_or(0) as u32,
        query_options: query_options(&obj),
        grafana_passthrough: passthrough(&obj),
        ..Cell::default()
    }
}

/// A stable cell key: prefer the Grafana numeric `id` (round-trips), else the panel index.
fn cell_key(obj: &Map<String, Value>, index: usize) -> String {
    match obj.get("id").and_then(Value::as_u64) {
        Some(id) => id.to_string(),
        None => index.to_string(),
    }
}

/// `gridPos{x,y,w,h}` → our grid (both 24-col per the spine). Missing → a reasonable default tile.
fn grid_pos(obj: &Map<String, Value>) -> (u32, u32, u32, u32) {
    let g = obj.get("gridPos");
    let get = |k: &str, d: u32| {
        g.and_then(|g| g.get(k))
            .and_then(Value::as_u64)
            .map(|n| n as u32)
            .unwrap_or(d)
    };
    (get("x", 0), get("y", 0), get("w", 12), get("h", 8))
}

/// `targets[]` → `sources[]`. Each Grafana target is one query; we carry `refId`, the (migrated)
/// `datasource` ref, and the whole target as `args` (opaque — `viz.query` re-checks per call). `tool`
/// is left empty HERE because the concrete MCP tool is only knowable once the caller BINDS the target's
/// datasource: [`super::bind`] fills it (plus the arg names that tool reads) on commit, right after
/// `datasources::apply`. A preview never binds — it writes nothing. `hide` maps to our `hide`.
fn targets_to_sources(obj: &Map<String, Value>) -> Vec<Target> {
    let panel_ds = obj.get("datasource").cloned().unwrap_or(Value::Null);
    obj.get("targets")
        .and_then(Value::as_array)
        .map(|targets| {
            targets
                .iter()
                .map(|t| {
                    let to = t.as_object().cloned().unwrap_or_default();
                    Target {
                        ref_id: to
                            .get("refId")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        // A target may carry its own datasource; else it inherits the panel's.
                        datasource: to
                            .get("datasource")
                            .cloned()
                            .unwrap_or_else(|| panel_ds.clone()),
                        tool: String::new(),
                        args: Value::Object(to),
                        hide: t.get("hide").and_then(Value::as_bool).unwrap_or(false),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Panel-level time override + query options (Grafana → our typed [`QueryOptions`]).
fn query_options(obj: &Map<String, Value>) -> crate::dashboard::model::QueryOptions {
    use crate::dashboard::model::QueryOptions;
    let s = |k: &str| obj.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    QueryOptions {
        max_data_points: obj
            .get("maxDataPoints")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        min_interval: s("interval"),
        relative_time: String::new(),
        time_from: s("timeFrom"),
        time_shift: s("timeShift"),
        hide_time_override: obj
            .get("hideTimeOverride")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    }
}

/// Everything we did NOT consume → the bounded `_grafana` passthrough (null if there's nothing left).
fn passthrough(obj: &Map<String, Value>) -> Value {
    let leftover: Map<String, Value> = obj
        .iter()
        .filter(|(k, _)| !CONSUMED_KEYS.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    if leftover.is_empty() {
        Value::Null
    } else {
        Value::Object(leftover)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn maps_supported_panel_fully() {
        let panel = json!({
            "id": 4,
            "type": "timeseries",
            "title": "CPU",
            "gridPos": {"x": 0, "y": 0, "w": 12, "h": 8},
            "datasource": {"uid": "prom"},
            "targets": [{"refId": "A", "expr": "up", "datasource": {"uid": "prom"}}],
            "fieldConfig": {"defaults": {"unit": "percent"}},
            "transformations": [{"id": "reduce"}],
            "someUnknownField": {"x": 1}
        });
        let mut degraded = vec![];
        let cell = panel_to_cell(&panel, 0, &mut degraded);
        assert_eq!(cell.i, "4");
        assert_eq!(cell.view, "timeseries");
        assert_eq!(cell.title, "CPU");
        assert_eq!((cell.x, cell.y, cell.w, cell.h), (0, 0, 12, 8));
        assert_eq!(cell.sources.len(), 1);
        assert_eq!(cell.sources[0].ref_id, "A");
        assert_eq!(cell.field_config, json!({"defaults": {"unit": "percent"}}));
        assert_eq!(cell.transformations.len(), 1);
        // unknown field preserved for round-trip
        assert_eq!(
            cell.grafana_passthrough,
            json!({"someUnknownField": {"x": 1}})
        );
        assert!(degraded.is_empty());
    }

    #[test]
    fn unsupported_panel_degrades_to_json_placeholder() {
        let panel = json!({"id": 7, "type": "heatmap", "title": "Heat"});
        let mut degraded = vec![];
        let cell = panel_to_cell(&panel, 0, &mut degraded);
        assert_eq!(cell.view, "json");
        assert_eq!(cell.options["unsupportedType"], json!("heatmap"));
        assert_eq!(degraded.len(), 1);
        assert_eq!(degraded[0].kind, "panel");
        assert!(degraded[0].detail.contains("heatmap"));
    }

    #[test]
    fn panel_without_id_keys_by_index() {
        let panel = json!({"type": "stat", "title": "X"});
        let cell = panel_to_cell(&panel, 5, &mut vec![]);
        assert_eq!(cell.i, "5");
    }
}
