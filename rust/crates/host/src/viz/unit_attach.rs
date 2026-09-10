//! Source-unit attachment — stamp a resolved frame's value columns with the series' registered unit
//! (`series_meta.unit`), so a client knows what the numbers it just received are IN.
//!
//! # Why the frame is the right place
//!
//! `lb_prefs` ships a correct uom-backed converter, and a `Field` can now carry a unit — but a
//! client still has to LEARN the unit from somewhere, and `series.list` returning bare strings meant
//! a second round trip per series just to find out. The frame already crosses the wire carrying the
//! values; carrying their unit alongside costs one registry read per target and makes the frame
//! self-describing.
//!
//! # What it does NOT do
//!
//! It does not CONVERT. Values stay canonical, exactly as the `Frame` header promises; the unit is
//! provenance, and whoever renders decides what to show it in. Converting here would need the
//! viewer's prefs threaded into the resolver and would bake a display decision into cached data.
//!
//! Bounded on purpose:
//!   - only a target whose args name a single `series` (the `series.read`/`series.latest` shape) —
//!     a `federation.query` over raw SQL has no series identity to look up, and guessing one would
//!     be worse than leaving the unit absent;
//!   - only NUMBER fields, and never the time column: a unit on an instant is meaningless;
//!   - a missing/unparseable registry unit leaves every field untouched (absent = "unknown", which
//!     is what every series that predates the column reports).

use lb_store::Store;
use lb_viz::{FieldType, Frame};
use serde_json::Value;

/// The series a target's args name, if they name exactly one. Mirrors the `series.read` arg shape;
/// anything else (a SQL target, a multi-series read) yields `None`.
pub fn target_series(args: &Value) -> Option<&str> {
    args.get("series")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

/// Stamp `frame`'s numeric, non-time fields with the registered unit of the series `args` names.
/// A no-op when the target names no series, the series has no registered unit, or the read fails —
/// provenance is a bonus, never a reason to fail a panel that already has its data.
pub async fn attach_unit(store: &Store, ws: &str, args: &Value, frame: &mut Frame) {
    let Some(series) = target_series(args) else {
        return;
    };
    let Ok(Some(unit)) = lb_ingest::unit(store, ws, series).await else {
        return;
    };
    let token = unit.as_str();
    for f in &mut frame.fields {
        if f.ty == FieldType::Number && f.unit.is_none() {
            f.unit = Some(token.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_series_target_names_its_series() {
        assert_eq!(
            target_series(&json!({ "series": "meter.main", "from": 1 })),
            Some("meter.main")
        );
    }

    /// A SQL target has no series identity — the host must not invent one.
    #[test]
    fn a_non_series_target_names_nothing() {
        assert_eq!(
            target_series(&json!({ "sql": "SELECT 1", "kind": "postgres" })),
            None
        );
        assert_eq!(target_series(&json!({ "series": "" })), None);
        assert_eq!(target_series(&json!({})), None);
    }
}
