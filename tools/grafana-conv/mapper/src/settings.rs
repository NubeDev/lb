//! Dashboard-level settings → `Dashboard` fields (grafana-conversion scope,
//! "Other dashboard-level features"). The "Have" rows map 1:1 here; the
//! "Degrade"/"Out" rows produce report lines, never a silent drop.

use crate::input::GrafanaDashboard;
use crate::report::ConversionReport;
use serde_json::Value;

/// The post-conversion summary of dashboard-level fields (what landed where).
pub struct DashSettings {
    pub id: String,
    pub title: String,
    pub description: String,
}

/// Map the dashboard-level fields. Returns the partial `Dashboard` inputs the
/// `convert` driver splices in (`id`/`title`/`description`/`schema_version` —
/// the rest are field-by-field elsewhere) plus report lines for everything else.
pub fn map_settings(dash: &GrafanaDashboard, report: &mut ConversionReport) -> DashSettings {
    report.mapped("dashboard.title", "", "title → Dashboard.title");
    if !dash.description.is_empty() {
        report.mapped(
            "dashboard.description",
            "",
            "description → Dashboard.description",
        );
    }
    if !dash.tags.is_empty() {
        report.mapped("dashboard.tags", "", "tags carried on the record");
    }
    if let Some(t) = non_empty_obj(&dash.time) {
        report.mapped(
            "dashboard.time",
            "",
            format!("time range {} carried on routing", short_json(&t)),
        );
    }
    if let Some(r) = non_empty_str(&dash.refresh) {
        report.mapped(
            "dashboard.refresh",
            "",
            format!("auto-refresh `{r}` carried on routing"),
        );
    }
    if let Some(p) = non_empty_obj(&dash.timepicker) {
        report.mapped(
            "dashboard.timepicker",
            "",
            format!("timepicker config carried as {}", short_json(&p)),
        );
    }

    // Degrade: schemaVersion (no migration in this cut).
    if dash.schema_version != 0 && dash.schema_version != 42 {
        report.degraded(
            "dashboard.schemaVersion",
            "",
            format!(
                "schemaVersion {} read as-is; no migration path",
                dash.schema_version
            ),
        );
    }
    // Degrade: timezone / weekStart / fiscalYear (partial — only timezone to user-prefs).
    if non_empty_str(&dash.timezone).is_some() {
        report.degraded(
            "dashboard.timezone",
            "",
            "timezone → user-prefs where present; not carried on the dashboard record",
        );
    }
    if !is_default_value(&dash.week_start) || !is_default_value(&dash.fiscal_year_start_month) {
        report.dropped(
            "dashboard.calendar",
            "",
            "weekStart / fiscalYearStartMonth dropped (no calendar config on the record)",
        );
    }
    // Degrade: liveNow / preload / editable (dropped-with-notice).
    if dash.live_now {
        report.dropped(
            "dashboard.liveNow",
            "",
            "liveNow dropped (no live-now mode)",
        );
    }
    if non_empty_obj(&dash.preload).is_some() {
        report.dropped("dashboard.preload", "", "preload dropped (no preload mode)");
    }
    if !dash.editable {
        report.dropped(
            "dashboard.editable",
            "",
            "editable=false dropped (dashboards are always editable here)",
        );
    }
    // Out: annotations / links / graphTooltip (named decisions).
    if non_empty_obj(&dash.annotations).is_some() {
        report.dropped("annotations", "", "no annotation plane");
    }
    if non_empty_obj(&dash.links).is_some() {
        report.dropped(
            "dashboard.links",
            "",
            "nav owns links, not the dashboard record",
        );
    }
    if non_empty_obj(&dash.graph_tooltip).is_some() {
        report.dropped("graphTooltip", "", "no shared-crosshair cursor sync");
    }

    DashSettings {
        id: slugify(&dash.title),
        title: dash.title.clone(),
        description: dash.description.clone(),
    }
}

fn non_empty_str(v: &Value) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

fn is_default_value(v: &Value) -> bool {
    matches!(v, Value::Null)
        || matches!(v, Value::String(s) if s.is_empty())
        || matches!(v, Value::Array(a) if a.is_empty())
        || matches!(v, Value::Object(o) if o.is_empty())
        || matches!(v, Value::Bool(b) if !b)
        || (v.is_number() && v.as_f64() == Some(0.0))
}

fn non_empty_obj(v: &Value) -> Option<Value> {
    match v {
        Value::Null => None,
        Value::String(s) if s.is_empty() => None,
        Value::Array(a) if a.is_empty() => None,
        Value::Object(o) if o.is_empty() => None,
        other => Some(other.clone()),
    }
}

fn short_json(v: &Value) -> String {
    let s = serde_json::to_string(v).unwrap_or_default();
    if s.len() > 40 {
        format!("{}…", &s[..40])
    } else {
        s
    }
}

fn slugify(s: &str) -> String {
    let slug: String = s
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "imported".into()
    } else {
        slug
    }
}
