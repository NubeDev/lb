//! `page_meta_of` — read a pack's dashboard JSON into the [`PageMeta`] a `dashboard.save` takes.
//!
//! One responsibility, and its own file because it is the ONE place a pack's page-settings
//! vocabulary is spelled out: a pack declares page settings with the SAME keys the settings dialog
//! sends, and with the same preserve-on-omit meaning — an absent key keeps the stored value, so
//! re-applying a pack never blanks page chrome an author has since set (nor silently demotes a
//! report back to a dashboard). Adding a page setting is one line here, beside the `dashboard.save`
//! schema it mirrors, instead of another line inside `apply.rs`'s already-long dashboard arm.

use serde_json::Value;

use crate::dashboard::PageMeta;

/// The page-settings meta a pack's dashboard object declares. Every field is preserve-on-omit: a key
/// the pack does not carry reads as `None` and leaves the stored value alone.
pub(super) fn page_meta_of(json: &Value) -> PageMeta {
    let str_key = |k: &str| json.get(k).and_then(Value::as_str).map(String::from);
    PageMeta {
        description: str_key("description"),
        heading: str_key("heading"),
        heading_size: str_key("headingSize"),
        show_heading: json.get("showHeading").and_then(Value::as_bool),
        icon: str_key("icon"),
        color: str_key("color"),
        timezone: str_key("timezone"),
        cache_ttl_s: json.get("cacheTtlS").and_then(Value::as_u64),
        toolbar: None,
        // Same keys, same preserve-on-omit: a pack page may declare a default window
        // (`"time": { "from": "last-7-days" }`); `dashboard_save_meta` validates it like any
        // other writer (a pack with a bad expression fails apply loudly, never stores a typo).
        time: json
            .get("time")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
        width: str_key("width"),
        compact: str_key("compact"),
        vars_display: str_key("varsDisplay"),
        kind: str_key("kind"),
        report_ids: json.get("reportIds").and_then(Value::as_array).map(|r| {
            r.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        }),
        // Same keys, same preserve-on-omit: a pack page may ship the export profiles its
        // report dialog offers.
        export_profiles: json
            .get("exportProfiles")
            .and_then(Value::as_array)
            .map(|r| {
                r.iter()
                    .filter_map(|v| serde_json::from_value(v.clone()).ok())
                    .collect()
            }),
    }
}
