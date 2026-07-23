//! Panel-type ↔ view-id alias table (viz import-export scope, the mapper's first row). Grafana's
//! `panel.type` and our [`Cell::view`](crate::dashboard::model::Cell) name the same render intent with
//! different vocabularies; this file is the one bidirectional translation. Import direction resolves a
//! Grafana type to a built-in view (or `None` = unsupported → the caller degrades honestly); export is
//! the inverse. The P3 `grafana-map` migration already renamed the legacy `graph`/`singlestat` types,
//! so by the time a panel reaches here it is the modern type — but we keep the legacy aliases too so a
//! caller that skips migration still maps.
//!
//! Only views in the shipped widget catalog are emitted on import — an unknown/unsupported type returns
//! `None` so the cell degrades to the honest `json` placeholder rather than a hallucinated view that
//! `check_view_cells` would reject on the commit `dashboard.save`.

/// Map a Grafana `panel.type` to our built-in `view` id. `None` = we don't support it (degrade).
/// The modern Grafana types map 1:1; the legacy names (pre-P3-migration) alias onto the same targets.
pub fn view_for_panel_type(panel_type: &str) -> Option<&'static str> {
    Some(match panel_type {
        // Modern Grafana types (post-migration) that we render natively.
        "timeseries" => "timeseries",
        "stat" => "stat",
        "gauge" => "gauge",
        "bargauge" => "bargauge",
        "table" => "table",
        "barchart" => "barchart",
        "piechart" => "piechart",
        "row" => "row",
        // A real `text`/markdown note maps to the shipped `text` view (sanitized markdown/html/code —
        // grafana-dashboard-fidelity slice 2). An EMPTY/logo text panel never reaches here — the
        // placeholder-drop (`report::drop_reason`) removes it first.
        "text" => "text",
        // Legacy aliases (a caller that imported without the P3 migration still resolves).
        "graph" => "timeseries",
        "singlestat" | "grafana-singlestat-panel" => "stat",
        "table-old" => "table",
        // Everything else — heatmap, logs, nodeGraph, a plugin panel — is unsupported here.
        _ => return None,
    })
}

/// Map our `view` id back to a Grafana `panel.type` on export. Every built-in view we import has a
/// canonical Grafana type; a view with no Grafana analogue (`plot`/`d3`/`template`/controls/`genui`/
/// `ext:*`) exports as its own name — Grafana won't render it, but the round-trip stays lossless (a
/// re-import degrades it honestly rather than the export silently dropping the cell).
pub fn panel_type_for_view(view: &str) -> &str {
    match view {
        "timeseries" => "timeseries",
        "stat" => "stat",
        "gauge" => "gauge",
        "bargauge" => "bargauge",
        "table" => "table",
        "barchart" => "barchart",
        "piechart" => "piechart",
        "row" => "row",
        // Our `text` view is Grafana's `text` panel (round-trips a converted note back).
        "text" => "text",
        // No Grafana analogue — carry our own view name so a round-trip is stable.
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modern_types_map_and_round_trip() {
        for (ty, view) in [
            ("timeseries", "timeseries"),
            ("stat", "stat"),
            ("gauge", "gauge"),
            ("table", "table"),
            ("piechart", "piechart"),
            ("row", "row"),
        ] {
            assert_eq!(view_for_panel_type(ty), Some(view));
            assert_eq!(panel_type_for_view(view), ty);
        }
    }

    #[test]
    fn legacy_aliases_import() {
        assert_eq!(view_for_panel_type("graph"), Some("timeseries"));
        assert_eq!(view_for_panel_type("singlestat"), Some("stat"));
        assert_eq!(view_for_panel_type("table-old"), Some("table"));
    }

    #[test]
    fn unsupported_type_is_none() {
        assert_eq!(view_for_panel_type("heatmap"), None);
        assert_eq!(view_for_panel_type("nodeGraph"), None);
        assert_eq!(view_for_panel_type("logs"), None);
    }

    #[test]
    fn text_maps_to_the_text_view_and_round_trips() {
        assert_eq!(view_for_panel_type("text"), Some("text"));
        assert_eq!(panel_type_for_view("text"), "text");
    }

    #[test]
    fn view_without_grafana_analogue_carries_own_name() {
        assert_eq!(panel_type_for_view("plot"), "plot");
        assert_eq!(panel_type_for_view("ext:mqtt/gauge"), "ext:mqtt/gauge");
    }
}
