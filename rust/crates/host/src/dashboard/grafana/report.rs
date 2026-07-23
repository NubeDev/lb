//! Placeholder-drop detection + dashboard-level degrade reporting (viz grafana-dashboard-fidelity
//! scope, slice 1). Two honesty jobs the mapper's per-panel path can't do alone:
//!
//! 1. **Drop decorative placeholders** instead of carrying them as empty "no template" tiles. Grafana's
//!    default `metric_table` banner-stat query, an empty/logo `text` panel, and a `dashlist` have no
//!    data and no host equivalent — mapping them to a cell renders an ugly empty box. We drop them (emit
//!    NO cell) and name each in the report.
//! 2. **Report the dashboard-level drops** the panel loop never sees: the annotation plane, the
//!    `refresh` interval, `graphTooltip`. Per the tool's honesty contract every drop is a report line —
//!    a dropped feature with no line is a test failure.
//!
//! Bounded + rule-10: detection keys off Grafana's own literal scaffold identifiers / panel `type`s,
//! never one of OUR extension ids, and never rewrites a query — it only decides "carry or drop, and say
//! so".

use serde_json::Value;

use super::DegradedItem;

/// If this panel is a decorative placeholder with no data and no host home, return the human reason to
/// DROP it (emit no cell); else `None` (map it normally). A `text` panel with real content is NOT a
/// placeholder — it maps to the `text` view; only an empty/logo one drops.
pub fn drop_reason(panel: &Value) -> Option<String> {
    let ty = panel.get("type").and_then(Value::as_str).unwrap_or("");
    match ty {
        "dashlist" => Some("dashlist (navigation panel) — no data, dropped".to_string()),
        "text" if text_is_decorative(panel) => {
            Some("empty/decorative text panel — dropped".to_string())
        }
        _ if has_default_placeholder_query(panel) => Some(
            "Grafana default placeholder query (metric_table) — decorative, no data, dropped"
                .to_string(),
        ),
        _ => None,
    }
}

/// A `text` panel is decorative when its `options.content` has no readable text — empty, whitespace, or
/// only markup (a logo `<div style="background-image:…">` with no words). We strip tags + whitespace and
/// call it decorative if nothing is left.
fn text_is_decorative(panel: &Value) -> bool {
    let content = panel
        .get("options")
        .and_then(|o| o.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("");
    strip_markup(content).trim().is_empty()
}

/// Remove `<...>` tags so we can tell a words-bearing note from a bare markup shell. Not a sanitizer
/// (the UI's `text` view owns sanitization) — just a decorative-vs-real test.
fn strip_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// True when a panel's only query is Grafana's default table/stat scaffold — the `metric_table` /
/// `time_column` placeholder Grafana inserts for an unconfigured SQL panel. Bounded literal match on
/// those scaffold identifiers (they are Grafana's, not a user's table name by coincidence at panel-
/// default time).
fn has_default_placeholder_query(panel: &Value) -> bool {
    let Some(targets) = panel.get("targets").and_then(Value::as_array) else {
        return false;
    };
    if targets.is_empty() {
        return false;
    }
    targets.iter().all(|t| {
        let sql = ["rawSql", "rawQuery", "query"]
            .iter()
            .find_map(|k| t.get(*k).and_then(Value::as_str))
            .unwrap_or("");
        let sql = sql.to_ascii_lowercase();
        sql.contains("metric_table") || sql.contains("time_column")
    })
}

/// A stable degrade key for a dropped panel (its Grafana id, else a blank — the report line carries the
/// detail either way).
pub fn dropped_panel_key(panel: &Value) -> String {
    panel
        .get("id")
        .and_then(Value::as_u64)
        .map(|n| n.to_string())
        .unwrap_or_default()
}

/// Report the dashboard-level features we do not import — collected from the raw JSON, appended to the
/// degraded list so nothing is silently dropped (the honesty contract).
pub fn dashboard_drops(json: &Value) -> Vec<DegradedItem> {
    let mut out = Vec::new();

    // Annotation plane — surfaced as a datasource to bind, but the annotation QUERIES themselves are not
    // imported (scope non-goal). Count non-builtin entries so a board with only the built-in still says so.
    if let Some(list) = json
        .get("annotations")
        .and_then(|a| a.get("list"))
        .and_then(Value::as_array)
    {
        if !list.is_empty() {
            out.push(DegradedItem {
                kind: "annotation".to_string(),
                cell: String::new(),
                detail: format!(
                    "annotation plane not imported ({} annotation quer{} dropped)",
                    list.len(),
                    if list.len() == 1 { "y" } else { "ies" }
                ),
            });
        }
    }

    // Auto-refresh interval — the toolbar refresh CONTROL is enabled by the verb, but the specific
    // interval (`30s`) is not carried onto the record.
    if let Some(refresh) = json.get("refresh").and_then(Value::as_str) {
        if !refresh.is_empty() {
            out.push(DegradedItem {
                kind: "refresh".to_string(),
                cell: String::new(),
                detail: format!(
                    "auto-refresh interval '{refresh}' not carried — the toolbar refresh control is enabled; pick a rate"
                ),
            });
        }
    }

    // Shared crosshair / tooltip linking across panels — no host equivalent.
    if json
        .get("graphTooltip")
        .and_then(Value::as_u64)
        .is_some_and(|v| v != 0)
    {
        out.push(DegradedItem {
            kind: "dashboard".to_string(),
            cell: String::new(),
            detail: "shared crosshair/tooltip (graphTooltip) not imported".to_string(),
        });
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dashlist_is_always_dropped() {
        assert!(drop_reason(&json!({"type": "dashlist"})).is_some());
    }

    #[test]
    fn an_empty_or_logo_text_panel_is_dropped_but_a_note_is_kept() {
        // empty content → dropped
        assert!(drop_reason(&json!({"type": "text", "options": {"content": "   "}})).is_some());
        // a bare logo div (markup, no words) → dropped
        let logo = json!({"type": "text", "options": {"content": "<div style=\"background-image:url(x)\"></div>"}});
        assert!(drop_reason(&logo).is_some());
        // a real note → kept (maps to the text view)
        let note = json!({"type": "text", "options": {"content": "# Level 2\nMonitoring notes"}});
        assert!(drop_reason(&note).is_none());
    }

    #[test]
    fn a_grafana_default_metric_table_stat_is_dropped() {
        let banner = json!({
            "type": "stat",
            "targets": [{"rawSql": "SELECT value FROM metric_table WHERE $__timeFilter(time_column)"}]
        });
        assert!(drop_reason(&banner).is_some());
    }

    #[test]
    fn a_real_stat_query_is_not_dropped() {
        let real = json!({
            "type": "stat",
            "targets": [{"rawSql": "SELECT value FROM histories WHERE point_uuid='p1' ORDER BY timestamp DESC LIMIT 1"}]
        });
        assert!(drop_reason(&real).is_none());
    }

    #[test]
    fn dashboard_drops_name_annotations_refresh_and_tooltip() {
        let json = json!({
            "annotations": {"list": [{"name": "Annotations & Alerts"}]},
            "refresh": "30s",
            "graphTooltip": 1
        });
        let drops = dashboard_drops(&json);
        assert_eq!(drops.len(), 3);
        assert!(drops.iter().any(|d| d.kind == "annotation"));
        assert!(drops
            .iter()
            .any(|d| d.kind == "refresh" && d.detail.contains("30s")));
        assert!(drops.iter().any(|d| d.detail.contains("graphTooltip")));
    }

    #[test]
    fn a_clean_dashboard_reports_no_dashboard_level_drops() {
        assert!(dashboard_drops(&json!({"panels": []})).is_empty());
    }
}
