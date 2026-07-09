//! Panels → `Cell` mapping (grafana-conversion scope, "Panels / layout" + trap #1).
//!
//! Two encodings of a row exist in classic Grafana JSON (the #1 corruption source
//! the audit surfaces):
//!
//! 1. **Collapsed** — children nested in `row.panels[]`, with `collapsed:true`.
//! 2. **Expanded** — `row.panels[]` empty; children are flat siblings identified
//!    only by grid `y` being greater than the row's `y` (and below the next row).
//!
//! We normalize **both** to a single shape: one `view:"row"` cell per row, with
//! the children emitted as ordinary flat `cells[]` siblings of the row cell (the
//! shape the panel-rows scope already ships). The row cell owns the title + the
//! collapsed flag; membership is by the row's grid span, not a nested array — so
//! the expanded/collapsed distinction collapses at storage time and the renderer
//! treats them uniformly.

use crate::input::{GrafanaDashboard, Panel};
use crate::model::{Cell, Target};
use crate::report::ConversionReport;
use serde_json::Value;

/// Map every classic `panels[]` entry to a flat list of `Cell`s (rows + their
/// children, normalized). Row dual-encoding is collapsed here.
pub fn map_cells(dash: &GrafanaDashboard, report: &mut ConversionReport) -> Vec<Cell> {
    let mut cells = Vec::new();
    for (idx, panel) in dash.panels.iter().enumerate() {
        let at = format!("panels[{idx}]");
        if panel.is_row() {
            map_row(panel, idx, &mut cells, report);
        } else {
            cells.push(map_panel_cell(panel, &at, report));
        }
    }
    cells
}

/// Map a `type:"row"` panel: emit the row cell itself, then (collapsed) its
/// nested children as flat siblings. Expanded rows contribute only themselves —
/// their children are already flat siblings in `dash.panels[]`.
fn map_row(row: &Panel, idx: usize, cells: &mut Vec<Cell>, report: &mut ConversionReport) {
    let at = format!("panels[{idx}]");
    let row_cell = Cell {
        i: row_cell_id(row),
        x: row.grid_pos.x,
        y: row.grid_pos.y,
        w: if row.grid_pos.w == 0 {
            24
        } else {
            row.grid_pos.w
        },
        h: if row.grid_pos.h == 0 {
            1
        } else {
            row.grid_pos.h
        },
        v: 3,
        view: "row".into(),
        title: row.title.clone(),
        ..Cell::default()
    };
    cells.push(row_cell);
    report.mapped("panel.row", &at, "row panel → view:\"row\" cell");

    // Collapsed row: children are nested; lift them to flat siblings.
    for (ci, child) in row.panels.iter().enumerate() {
        let child_at = format!("{at}.panels[{ci}]");
        cells.push(map_panel_cell(child, &child_at, report));
    }

    // Repeat-by on a row degrades (panel-rows scope follow-up + multi-value vars).
    if !is_null_or_empty(&row.repeat) {
        report.degraded(
            "panel.row.repeat",
            &at,
            "row repeat preserved as raw `repeat`; not rendered (multi-value vars follow-up)",
        );
    }
}

/// Map a non-row panel to a `Cell`. Panel type is **opaque data** (rule 10): a
/// type we can't classify becomes a reported degrade (template placeholder), never
/// a branch on one of our extension ids.
fn map_panel_cell(panel: &Panel, at: &str, report: &mut ConversionReport) -> Cell {
    let cell = Cell {
        i: panel_cell_id(panel),
        x: panel.grid_pos.x,
        y: panel.grid_pos.y,
        w: grid_default(panel.grid_pos.w, 6),
        h: grid_default(panel.grid_pos.h, 8),
        v: 3,
        view: classify_view(&panel.r#type, report, at),
        title: panel.title.clone(),
        description: panel.description.clone(),
        sources: map_targets(panel, report, at),
        transformations: panel.transformations.clone(),
        field_config: panel.field_config.clone(),
        options: panel.options.clone(),
        plugin_version: value_to_string(&panel.plugin_version),
        panel_ref: library_ref(&panel.library_panel, report, at),
        ..Cell::default()
    };

    // Repeat-by on a panel degrades (depends on multi-value vars).
    if !is_null_or_empty(&panel.repeat) {
        report.degraded(
            "panel.repeat",
            at,
            "panel repeat preserved as raw `repeat`; not rendered (multi-value vars follow-up)",
        );
    }

    report.mapped("panel.grid", at, "24-col gridPos → x/y/w/h");
    cell
}

/// Map Grafana `targets[]` → `Cell.sources[]` (the v3 `Target`). Datasource UIDs
/// are carried **opaque** (grafana-conversion scope, "Risks"): no UID → our-
/// datasource mapping exists in this cut, so each target is reported
/// `datasource.unresolved`.
fn map_targets(panel: &Panel, report: &mut ConversionReport, at: &str) -> Vec<Target> {
    panel
        .targets
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let t_at = format!("{at}.targets[{i}]");
            let ref_id = t
                .get("refId")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| letter_label(i));
            let datasource = t
                .get("datasource")
                .cloned()
                .unwrap_or_else(|| panel.datasource.clone());
            if !datasource.is_null() {
                report.degraded(
                    "datasource.uid",
                    &t_at,
                    "Grafana datasource UID carried opaque; no federation mapping yet",
                );
            }
            Target {
                ref_id,
                datasource,
                args: t.clone(),
                hide: t.get("hide").and_then(Value::as_bool).unwrap_or(false),
                ..Target::default()
            }
        })
        .collect()
}

/// Map a `libraryPanel` ref → `panelRef` (`panel:{uid}` — library-panels scope).
fn library_ref(lib: &Value, report: &mut ConversionReport, at: &str) -> String {
    if lib.is_null() {
        return String::new();
    }
    let uid = lib.get("uid").and_then(Value::as_str).unwrap_or("");
    if uid.is_empty() {
        return String::new();
    }
    report.mapped(
        "panel.libraryPanel",
        at,
        "library-panel ref → panelRef cell field",
    );
    format!("panel:{uid}")
}

/// The view vocabulary. Grafana `panel.type` → our `view`. The mapping is **not**
/// a branch on one of our extension ids — `panel.type` is opaque input data. An
/// unmapped type degrades to `view:"template"` (the honest placeholder), reported.
fn classify_view(panel_type: &str, report: &mut ConversionReport, at: &str) -> String {
    let view = match panel_type {
        "timeseries" | "graph" => "chart",
        "stat" => "stat",
        "gauge" => "gauge",
        "table" => "table",
        "row" => "row",
        "" => {
            report.degraded(
                "panel.type.unknown",
                at,
                "panel has no `type`; rendered as a template placeholder",
            );
            return "template".into();
        }
        other => {
            report.degraded(
                "panel.type.unknown",
                at,
                format!("panel type `{other}` has no 1:1 view; rendered as a template placeholder"),
            );
            return "template".into();
        }
    };
    report.mapped(
        "panel.type",
        at,
        format!("`{panel_type}` → view:\"{view}\""),
    );
    view.into()
}

fn grid_default(v: u32, def: u32) -> u32 {
    if v == 0 {
        def
    } else {
        v
    }
}

fn value_to_string(v: &Value) -> String {
    v.as_str().map(str::to_string).unwrap_or_default()
}

fn is_null_or_empty(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.is_empty(),
        _ => false,
    }
}

fn letter_label(i: usize) -> String {
    let c = (b'A' + i as u8) as char;
    c.to_string()
}

fn panel_cell_id(panel: &Panel) -> String {
    match panel.id {
        Some(id) => format!("panel-{id}"),
        None => format!("panel-{}-{}", panel.grid_pos.x, panel.grid_pos.y),
    }
}

fn row_cell_id(row: &Panel) -> String {
    match row.id {
        Some(id) => format!("row-{id}"),
        None => "row".to_string(),
    }
}
