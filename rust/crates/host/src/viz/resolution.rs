//! Panel-resolution negotiation for `viz.query` (viz panel-resolution scope, issue #101). Turns a
//! panel's **visible time range** + **point budget** into one snapped **bucket width**, then upgrades
//! the target in place:
//!
//!   - a mode-less `series.read` target gains `{mode:"buckets", from, to, width_ms}` — the shipped
//!     decimation path ([`crate::ingest`] / `lb_ingest::read_buckets`), spike-safe `{t,min,max,avg,
//!     last,count}` buckets, rollup-tier merge included. No engine work: the width is the only new bit.
//!   - a `federation.query` target's `$__interval`/`$__interval_ms`/`$__timeFrom`/`$__timeTo` macros
//!     ([`super::macros`]) are substituted with the SAME derived values, so a hand-written
//!     `date_bin(INTERVAL '$__interval', …)` coarsens itself as the range grows.
//!
//! **The width is a pure function of `(range, budget, min_interval)`** — deterministic ⇒ N viewers of
//! the same panel derive byte-identical args ⇒ the quantised cache keys (dashboard-query-acceleration)
//! collapse to one compute. The derivation is the whole contract; the injection is one call each in
//! `dispatch_target` (query.rs is already at the FILE-LAYOUT line, so the bodies live here).
//!
//! **Grafana's model, not a fixed ladder.** `width = ceil(range / budget)`, snapped **up** to a human
//! step ladder (a budget is a ceiling, never exceeded), floored by `minInterval` (never finer than the
//! data's cadence), clamped to the engine's `MAX_BUCKETS`. Resolution scales with the range for free.

use serde_json::{json, Value};

use crate::dashboard::QueryOptions;

/// The human step ladder (ms), ascending: `1s 5s 10s 30s 1m 5m 10m 15m 30m 1h 2h 3h 6h 12h 1d 7d 30d`.
/// Snap **UP** — the derived width is the first ladder step ≥ the ideal `range/budget`, so the bucket
/// count is always ≤ budget. Above the top step, whole multiples of 30d (the range is astronomical;
/// still deterministic).
const LADDER_MS: &[u64] = &[
    1_000,         // 1s
    5_000,         // 5s
    10_000,        // 10s
    30_000,        // 30s
    60_000,        // 1m
    300_000,       // 5m
    600_000,       // 10m
    900_000,       // 15m
    1_800_000,     // 30m
    3_600_000,     // 1h
    7_200_000,     // 2h
    10_800_000,    // 3h
    21_600_000,    // 6h
    43_200_000,    // 12h
    86_400_000,    // 1d
    604_800_000,   // 7d
    2_592_000_000, // 30d
];

/// Hard ceiling on buckets per read — MUST mirror `lb_ingest::bucket::MAX_BUCKETS` (2 000). The
/// bucket engine REJECTS a `width_ms` that would yield more than this, so the derivation clamps to it:
/// a huge budget or a tiny minInterval can never produce an over-cap width that the engine then errors
/// on. (Kept as a local const rather than a cross-crate import so `resolution.rs` stays pure-math and
/// testable without the store; the integration tests prove the two agree.)
pub const MAX_BUCKETS: u64 = 2_000;

/// The default point budget when a panel authors no `maxDataPoints` — matches the shipped decimation
/// examples and the ~1 000-px chart the dashboard draws into (scope open question, resolved: fixed).
pub const DEFAULT_BUDGET: u64 = 1_000;

/// Derive the snapped bucket **width in ms** for a window.
///
/// - `range_ms` = `to - from` (the visible span). A zero/absent span returns `None` — the caller
///   refuses to inject (no window ⇒ nothing to bucket), never a divide-by-zero or a bogus width.
/// - `budget` = max points for the window (0 ⇒ [`DEFAULT_BUDGET`]).
/// - `min_interval_ms` = the panel's `minInterval` floor (0 ⇒ none) — the width is never finer than
///   this (a 15-min sensor should never bucket at 1 s).
///
/// The result satisfies **all three** simultaneously: it is a ladder step (or a 30d multiple) ≥
/// `ceil(range/budget)`, ≥ `min_interval_ms`, and coarse enough that `range/width ≤ MAX_BUCKETS`.
pub fn derive_width(range_ms: u64, budget: u64, min_interval_ms: u64) -> Option<u64> {
    if range_ms == 0 {
        return None;
    }
    let budget = if budget == 0 { DEFAULT_BUDGET } else { budget };

    // Ideal width to hit the budget exactly, rounded UP so we never exceed it.
    let ideal = range_ms.div_ceil(budget);

    // Snap up to the ladder, then apply the two floors (both raise the width; take the max):
    //   1. minInterval — the author's cadence floor (raw, not re-snapped: it is typically already the
    //      data's true cadence, e.g. "15m"; snapping it would coarsen past the author's intent).
    //   2. MAX_BUCKETS — the engine's hard ceiling: widen until range/width ≤ MAX_BUCKETS.
    let mut width = snap_up(ideal);
    width = width.max(min_interval_ms);
    let cap_floor = range_ms.div_ceil(MAX_BUCKETS);
    width = width.max(cap_floor);

    Some(width.max(1))
}

/// The first ladder step ≥ `ideal`; above the ladder, the smallest whole multiple of the top step.
fn snap_up(ideal: u64) -> u64 {
    for &step in LADDER_MS {
        if step >= ideal {
            return step;
        }
    }
    let top = *LADDER_MS.last().expect("ladder is non-empty");
    top.saturating_mul(ideal.div_ceil(top))
}

/// The effective point budget for a panel — the authored `maxDataPoints` or [`DEFAULT_BUDGET`].
pub fn effective_budget(qo: &QueryOptions) -> u64 {
    if qo.max_data_points == 0 {
        DEFAULT_BUDGET
    } else {
        qo.max_data_points
    }
}

/// Parse the panel's `minInterval` duration string (`"10s"`, `"15m"`, `"1h"`, `"1d"` …) to **ms**.
/// Grafana's fixed-amount units (`M` = 30 d, `y` = 365 d), matching `time_override`'s grammar but in
/// ms. Empty / unparsable ⇒ `0` (no floor) — a bad knob degrades to "no minimum", never a failed panel.
pub fn parse_min_interval_ms(s: &str) -> u64 {
    let s = s.trim();
    if s.is_empty() {
        return 0;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let Ok(n) = num.parse::<u64>() else {
        return 0;
    };
    let mult_ms: u64 = match unit {
        "s" => 1_000,
        "m" => 60_000,
        "h" => 3_600_000,
        "d" => 86_400_000,
        "w" => 7 * 86_400_000,
        "M" => 30 * 86_400_000,
        "y" => 365 * 86_400_000,
        _ => return 0,
    };
    n.saturating_mul(mult_ms)
}

/// The derived resolution for one target's window: the (possibly override-adjusted) `[from, to]` in
/// epoch ms and the snapped `width_ms`. `None` when the target carries no numeric window (a bare
/// `{series}` read, a rows table, an export) — the caller leaves such a target untouched.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub from: u64,
    pub to: u64,
    pub width_ms: u64,
}

/// Compute the [`Resolution`] for a target from its `args` (a numeric `from`/`to`, epoch ms) and the
/// panel `queryOptions` (budget + minInterval). Returns `None` if `from`/`to` are absent, non-numeric,
/// or an inverted/empty window — the target then keeps today's behavior exactly.
pub fn resolution_for(args: &Value, qo: &QueryOptions) -> Option<Resolution> {
    let from = args.get("from").and_then(Value::as_u64)?;
    let to = args.get("to").and_then(Value::as_u64)?;
    if to <= from {
        return None;
    }
    let width_ms = derive_width(
        to - from,
        effective_budget(qo),
        parse_min_interval_ms(&qo.min_interval),
    )?;
    Some(Resolution { from, to, width_ms })
}

/// Upgrade a mode-less `series.read` target's `args` to the bucketed decimation path, in place.
///
/// Injects `{mode:"buckets", width_ms}` (the `from`/`to` already carry the window). **Explicit author
/// intent always wins**: a target that already sets `mode` (a `mode:"rows"` table/export/raw view) or
/// an explicit `width_ms` (an aligned multi-series overlay) is left byte-for-byte untouched. A target
/// with no numeric window is left alone (nothing to bucket). Returns `true` when it injected.
pub fn maybe_inject_buckets(args: &mut Value, qo: &QueryOptions) -> bool {
    let Value::Object(map) = args else {
        return false;
    };
    // Explicit mode or explicit width_ms → the author decided; never override.
    if map.contains_key("mode") || map.contains_key("width_ms") {
        return false;
    }
    let Some(res) = resolution_for(args, qo) else {
        return false;
    };
    let Value::Object(map) = args else {
        return false;
    };
    map.insert("mode".into(), json!("buckets"));
    map.insert("width_ms".into(), json!(res.width_ms));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the ladder & budget-ceiling table (the scope's headline: coarser-as-you-zoom-out) --------

    /// `derive_width` snaps UP to the ladder and the bucket count NEVER exceeds the budget — asserted
    /// at every range in the scope's table (6h→30s, 1mo→1h, 1y→12h, 2y→1d) plus the fine end.
    #[test]
    fn ladder_snaps_up_and_respects_the_budget() {
        const H: u64 = 3_600_000;
        const D: u64 = 86_400_000;
        let cases: &[(u64, u64, u64)] = &[
            // (range_ms, budget, expected_width_ms)
            (6 * H, 1_000, 30_000),       // 6h  → 30s   (~720 pts)
            (30 * D, 1_000, H),           // 1mo → 1h    (~720 pts)
            (365 * D, 1_000, 43_200_000), // 1y  → 12h   (~730 pts)
            (2 * 365 * D, 1_000, D),      // 2y  → 1d    (~730 pts)
            (H, 1_000, 5_000),            // 1h  → 5s    (3600/1000=3.6 → snap 5s)
            (60_000, 1_000, 1_000),       // 1m  → 1s    (floor of the ladder)
        ];
        for &(range, budget, want) in cases {
            let got = derive_width(range, budget, 0).unwrap();
            assert_eq!(got, want, "range={range}ms budget={budget}");
            // The budget is a hard ceiling — snapping UP can only REDUCE the count below budget.
            let buckets = range.div_ceil(got);
            assert!(
                buckets <= budget.max(1),
                "range={range} width={got} → {buckets} buckets > budget {budget}"
            );
        }
    }

    /// The budget is a ceiling at EVERY ladder step, for a swept range — never more points than asked.
    #[test]
    fn budget_is_a_hard_ceiling_across_the_sweep() {
        for exp in 0..40u32 {
            let range = 1_000u64.saturating_mul(1u64 << exp.min(40)); // 1s, 2s, 4s … huge
            for &budget in &[100u64, 500, 1_000, 2_000] {
                let w = derive_width(range, budget, 0).unwrap();
                let buckets = range.div_ceil(w);
                assert!(
                    buckets <= budget,
                    "range={range} budget={budget} width={w} → {buckets} buckets"
                );
            }
        }
    }

    /// `minInterval` FLOORS the width — a coarse sensor cadence overrides the budget-derived width.
    #[test]
    fn min_interval_floors_the_width() {
        const H: u64 = 3_600_000;
        // 6h/1000 → 30s, but a 5-min sensor floor lifts it to 5m.
        let w = derive_width(6 * H, 1_000, 300_000);
        assert_eq!(
            w,
            Some(300_000),
            "minInterval 5m floors above the 30s budget width"
        );
        // A minInterval BELOW the derived width does nothing.
        let w = derive_width(6 * H, 1_000, 1_000);
        assert_eq!(
            w,
            Some(30_000),
            "minInterval 1s is below the 30s width → no effect"
        );
    }

    /// `MAX_BUCKETS` clamps — a huge budget can never make the engine reject the width. The clamp
    /// wins even over a tiny minInterval.
    #[test]
    fn max_buckets_clamps_the_count() {
        const D: u64 = 86_400_000;
        // 1y with a 100k budget WANTS ~5min buckets (105k of them) — clamp to ≤ 2000.
        let range = 365 * D;
        let w = derive_width(range, 100_000, 0).unwrap();
        assert!(
            range.div_ceil(w) <= MAX_BUCKETS,
            "clamped to ≤{MAX_BUCKETS}, got {} buckets (width {w})",
            range.div_ceil(w)
        );
        // Even a 1ms minInterval cannot push the count over the cap.
        let w = derive_width(range, 100_000, 1).unwrap();
        assert!(range.div_ceil(w) <= MAX_BUCKETS);
    }

    /// A zero / inverted / empty window refuses cleanly (`None`) — no divide-by-zero, no bogus width.
    #[test]
    fn empty_or_inverted_window_refuses() {
        assert_eq!(derive_width(0, 1_000, 0), None, "zero range");
        // resolution_for guards the inversion.
        let qo = QueryOptions::default();
        assert_eq!(
            resolution_for(&json!({ "from": 100, "to": 50 }), &qo),
            None,
            "inverted"
        );
        assert_eq!(
            resolution_for(&json!({ "from": 50, "to": 50 }), &qo),
            None,
            "empty"
        );
        assert_eq!(
            resolution_for(&json!({ "series": "x" }), &qo),
            None,
            "no window"
        );
        assert_eq!(
            resolution_for(&json!({ "from": "now-1h", "to": "now" }), &qo),
            None,
            "non-numeric window (another tool's vocabulary) is left alone"
        );
    }

    /// Budget defaulting: an unset (0) budget uses [`DEFAULT_BUDGET`].
    #[test]
    fn zero_budget_defaults_to_1000() {
        const H: u64 = 3_600_000;
        assert_eq!(
            derive_width(6 * H, 0, 0),
            derive_width(6 * H, DEFAULT_BUDGET, 0)
        );
    }

    /// The duration grammar for `minInterval` (ms), Grafana's fixed units.
    #[test]
    fn min_interval_grammar() {
        assert_eq!(parse_min_interval_ms("10s"), 10_000);
        assert_eq!(parse_min_interval_ms("15m"), 900_000);
        assert_eq!(parse_min_interval_ms("1h"), 3_600_000);
        assert_eq!(parse_min_interval_ms("1d"), 86_400_000);
        assert_eq!(parse_min_interval_ms(""), 0);
        assert_eq!(parse_min_interval_ms("banana"), 0);
        assert_eq!(parse_min_interval_ms("1.5h"), 0);
    }

    // ---- the injection (explicit intent wins) ------------------------------------------------------

    /// A mode-less `series.read` with a numeric window gains `{mode:"buckets", width_ms}`; the window
    /// survives, the derived width matches `derive_width`.
    #[test]
    fn injects_buckets_into_mode_less_window() {
        const H: u64 = 3_600_000;
        let qo = QueryOptions::default();
        let mut args = json!({ "series": "cpu", "from": 0u64, "to": 6 * H });
        assert!(maybe_inject_buckets(&mut args, &qo));
        assert_eq!(args["mode"], json!("buckets"));
        assert_eq!(args["width_ms"], json!(30_000)); // 6h/1000 → 30s
        assert_eq!(args["from"], json!(0));
        assert_eq!(args["to"], json!(6 * H), "window untouched");
        assert_eq!(args["series"], json!("cpu"), "other args untouched");
    }

    /// Explicit `mode:"rows"` is left byte-for-byte alone (tables / exports / the raw inspector).
    #[test]
    fn explicit_rows_mode_is_untouched() {
        const H: u64 = 3_600_000;
        let qo = QueryOptions::default();
        let mut args = json!({ "series": "cpu", "from": 0u64, "to": 6 * H, "mode": "rows" });
        let before = args.clone();
        assert!(!maybe_inject_buckets(&mut args, &qo));
        assert_eq!(args, before, "explicit mode:rows wins");
    }

    /// An explicit `width_ms` wins — an aligned multi-series overlay keeps its author-chosen width.
    #[test]
    fn explicit_width_is_untouched() {
        const H: u64 = 3_600_000;
        let qo = QueryOptions::default();
        let mut args = json!({ "series": "cpu", "from": 0u64, "to": 6 * H, "width_ms": 12_345u64 });
        let before = args.clone();
        assert!(!maybe_inject_buckets(&mut args, &qo));
        assert_eq!(args, before, "explicit width_ms wins");
    }

    /// A target with no numeric window (a bare `{series}` read) is left alone — nothing to bucket.
    #[test]
    fn no_window_is_untouched() {
        let qo = QueryOptions::default();
        let mut args = json!({ "series": "cpu" });
        let before = args.clone();
        assert!(!maybe_inject_buckets(&mut args, &qo));
        assert_eq!(args, before);
    }

    /// The authored budget flows through: a bigger budget → finer width (down to the ladder).
    #[test]
    fn authored_budget_reaches_the_width() {
        const H: u64 = 3_600_000;
        let qo = QueryOptions {
            max_data_points: 100,
            ..Default::default()
        };
        let mut args = json!({ "series": "cpu", "from": 0u64, "to": 6 * H });
        assert!(maybe_inject_buckets(&mut args, &qo));
        // 6h/100 = 216s → snap up to 5m (300s).
        assert_eq!(args["width_ms"], json!(300_000));
    }
}
