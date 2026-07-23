//! The time-bucket **quantiser** for the `viz.query` gateway cache (dashboard-query-acceleration
//! scope, slice 2). `viz.query`'s resolved `now`/`from`/`to` are per-open (a relative "last 1h" board
//! computes a fresh range every open), so without quantisation two opens a second apart key
//! differently and NEVER share a warm entry. This floors those values to the class's **TTL bucket**
//! before canonicalisation, so opens within one bucket produce the SAME key — and the query runs on
//! the bucketed range (the key and the executed range agree; the cache never serves a range it didn't
//! compute).
//!
//! Bounded on purpose (mirrors `time_override.rs`): it floors only NUMERIC epoch values it finds —
//! the top-level `now`, and each target's numeric `from`/`to` (the `series.read` range vocabulary). A
//! target whose time lives inside a SQL string (`federation.query`) is untouched here; that range is
//! bucket-aligned by the caller before it reaches the wire (the UI half quantises the tokens it bakes
//! into SQL), so both layers land on the same grid. A non-windowed arg is never bucketed. Unit-detect:
//! a value at epoch-**ms** magnitude buckets in ms, epoch-**s** in seconds — so both range vocabularies
//! floor to a real wall-clock bucket, not a no-op.
//!
//! **End-day-exclusivity survives the floor** (`dashboard-time-range-tokens`): a `to` on a bucket
//! boundary (a day boundary is divisible by any sane sub-day bucket) floors to itself, so the
//! half-open `[from, to)` window is preserved — the quantiser never pulls an exclusive end inward.

use serde_json::{json, Value};

/// Above this magnitude a numeric epoch is treated as **milliseconds**, below it as **seconds**. Real
/// epoch-seconds today are ~1.7e9 (far below); real epoch-ms are ~1.7e12 (far above). 1e11 sits in the
/// empty gap between them for every plausible dashboard range, so the split is unambiguous.
const MS_MAGNITUDE_THRESHOLD: u64 = 100_000_000_000;

/// Return a copy of a `viz.query` input with its time values floored to the `ttl_s` bucket. Accepts
/// both the wrapped shape (`{panel, now, cache}`) and a bare panel (the input IS the panel).
pub fn quantise_viz_input(input: &Value, ttl_s: u64) -> Value {
    let bucket = ttl_s.max(1);
    let mut out = input.clone();

    if let Value::Object(map) = &mut out {
        if let Some(now) = map.get("now").and_then(Value::as_u64) {
            map.insert("now".into(), json!(floor_to_bucket(now, bucket)));
        }
        if let Some(panel) = map.get_mut("panel") {
            quantise_panel_ranges(panel, bucket);
            return out;
        }
    }
    // No `panel` key ⇒ the input itself is the panel (a bare `viz.query` call).
    quantise_panel_ranges(&mut out, bucket);
    out
}

/// Floor the numeric `from`/`to` in every target's args (both the v3 `sources[]` and the v2 single
/// `source`). Everything else on the panel is left byte-for-byte.
fn quantise_panel_ranges(panel: &mut Value, bucket_secs: u64) {
    if let Some(sources) = panel.get_mut("sources").and_then(Value::as_array_mut) {
        for s in sources.iter_mut() {
            quantise_args(s.get_mut("args"), bucket_secs);
        }
    }
    if let Some(source) = panel.get_mut("source") {
        quantise_args(source.get_mut("args"), bucket_secs);
    }
}

/// Floor a target's numeric `from`/`to` args to the bucket, in their own unit (ms or s). A missing
/// arg, a non-numeric one (another tool's string vocabulary), or non-object args are left untouched.
fn quantise_args(args: Option<&mut Value>, bucket_secs: u64) {
    let Some(Value::Object(map)) = args else {
        return;
    };
    for key in ["from", "to"] {
        if let Some(n) = map.get(key).and_then(Value::as_u64) {
            map.insert(key.into(), json!(floor_to_bucket(n, bucket_secs)));
        }
    }
}

/// Floor an epoch value to the TTL bucket, choosing the unit (ms vs s) by magnitude so both range
/// vocabularies land on a real wall-clock bucket boundary.
fn floor_to_bucket(t: u64, bucket_secs: u64) -> u64 {
    let unit = if t >= MS_MAGNITUDE_THRESHOLD {
        bucket_secs.saturating_mul(1000)
    } else {
        bucket_secs
    }
    .max(1);
    (t / unit) * unit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floors_top_level_now_in_seconds() {
        // now = 1_000_000 s, 60 s bucket → floor to 999_960 (16666 * 60).
        let out = quantise_viz_input(&json!({ "panel": {}, "now": 1_000_000u64 }), 60);
        assert_eq!(out["now"], json!(999_960u64));
    }

    #[test]
    fn floors_series_target_range_seconds() {
        let input = json!({
            "panel": { "sources": [{ "tool": "series.read", "args": { "series": "cpu", "from": 1_000_037u64, "to": 1_000_119u64 } }] },
            "now": 1_000_119u64,
        });
        let out = quantise_viz_input(&input, 60);
        // 1_000_037 // 60 * 60 = 1_000_020 ; 1_000_119 // 60 * 60 = 1_000_080
        assert_eq!(
            out["panel"]["sources"][0]["args"]["from"],
            json!(1_000_020u64)
        );
        assert_eq!(
            out["panel"]["sources"][0]["args"]["to"],
            json!(1_000_080u64)
        );
        // Non-range args are untouched.
        assert_eq!(out["panel"]["sources"][0]["args"]["series"], json!("cpu"));
    }

    #[test]
    fn floors_epoch_ms_range_in_ms_units() {
        // Epoch-ms magnitude → bucket in ms (60 s = 60_000 ms).
        //   from 1_700_000_037_000 // 60_000 * 60_000 = 1_699_999_980_000
        //   to   1_700_000_099_000 // 60_000 * 60_000 = 1_700_000_040_000
        let input = json!({
            "panel": { "source": { "tool": "series.read", "args": { "from": 1_700_000_037_000u64, "to": 1_700_000_099_000u64 } } },
        });
        let out = quantise_viz_input(&input, 60);
        assert_eq!(
            out["panel"]["source"]["args"]["from"],
            json!(1_699_999_980_000u64)
        );
        assert_eq!(
            out["panel"]["source"]["args"]["to"],
            json!(1_700_000_040_000u64)
        );
    }

    #[test]
    fn end_day_exclusive_boundary_survives_floor() {
        // A `to` exactly on a day boundary (midnight, epoch-ms) is divisible by a 60 s bucket → floors
        // to itself, so the exclusive end is never pulled inward.
        let midnight_ms = 1_700_006_400_000u64; // a real day boundary (divisible by 86_400_000)
        let input = json!({
            "panel": { "sources": [{ "tool": "series.read", "args": { "from": 1_699_920_000_000u64, "to": midnight_ms } }] },
        });
        let out = quantise_viz_input(&input, 60);
        assert_eq!(out["panel"]["sources"][0]["args"]["to"], json!(midnight_ms));
    }

    #[test]
    fn non_windowed_and_string_ranges_untouched() {
        let input = json!({
            "panel": { "sources": [{ "tool": "store.query", "args": { "sql": "SELECT 1", "from": "now-1h" } }] },
            "now": 500u64,
        });
        let out = quantise_viz_input(&input, 60);
        // String `from` (another tool's vocabulary) is left alone; sql untouched; now floored (500//60*60=480).
        assert_eq!(out["panel"]["sources"][0]["args"]["from"], json!("now-1h"));
        assert_eq!(out["panel"]["sources"][0]["args"]["sql"], json!("SELECT 1"));
        assert_eq!(out["now"], json!(480u64));
    }

    #[test]
    fn two_opens_in_one_bucket_produce_equal_quantised_input() {
        let a = json!({ "panel": { "source": { "tool": "series.read", "args": { "from": 1_000_001u64, "to": 1_000_050u64 } } }, "now": 1_000_050u64 });
        let b = json!({ "panel": { "source": { "tool": "series.read", "args": { "from": 1_000_009u64, "to": 1_000_058u64 } } }, "now": 1_000_058u64 });
        assert_eq!(quantise_viz_input(&a, 60), quantise_viz_input(&b, 60));
    }
}
