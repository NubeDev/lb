//! Panel time override for `viz.query` target dispatch (viz grafana-parity-backend scope, P1).
//! Grafana's `applyPanelTimeOverrides` semantics, pinned in the P1 session doc:
//!
//!  1. `timeFrom` **replaces** the range: `[now - timeFrom, now]` — an override, not a nudge.
//!  2. `timeShift` then moves **both** ends earlier by the shift (`from -= shift, to -= shift`).
//!  3. `hideTimeOverride` is display-only — it never touches the query.
//!
//! Bounded on purpose: the host applies the override only to a target's **numeric epoch-millisecond**
//! `from`/`to` args (the `series.read` contract — the one range vocabulary the platform has).
//! A target whose args carry no range and no `timeFrom` is left alone (`timeShift` has nothing to
//! shift); a non-numeric `from`/`to` (a string expression some ext tool owns) is left untouched —
//! the host never guesses another tool's vocabulary. An unparsable duration is a silent no-op
//! (degrade, never a failed panel). Pure — no store, no clock (the caller threads logical `now`).
//!
//! # The unit, and the bug that lived here
//!
//! Everything on this path is **epoch milliseconds**: `series.read`'s buckets mode rejects a `from`
//! that is not epoch ms (`ingest/tool.rs`), `viz::resolution` derives its bucket width from an
//! ms-valued `(to - from)`, and `$__timeFrom`/`$__timeTo` expand to ms.
//!
//! This module used to say — twice, in its own header — that it worked in epoch SECONDS, and its
//! duration parser returned seconds which were then subtracted straight from an ms clock. A panel
//! setting `timeFrom: "6h"` therefore asked for a **21.6-second** window instead of six hours: 1000×
//! too small, silently empty, never an error. It survived because no cell in a real 269-cell corpus
//! had ever set `timeFrom` or `timeShift`, so the live code path was never exercised.
//!
//! The parser now returns milliseconds and the module name says so.

use serde_json::{json, Value};

use crate::dashboard::QueryOptions;

/// Apply a panel's `timeFrom`/`timeShift` to one target's `args` before dispatch. `now` is the
/// caller's logical clock (epoch MILLISECONDS), the same one threaded as `ts`.
pub fn apply_time_override(args: &mut Value, qo: &QueryOptions, now: u64) {
    let time_from = parse_duration_ms(&qo.time_from);
    let time_shift = parse_duration_ms(&qo.time_shift);
    if time_from.is_none() && time_shift.is_none() {
        return;
    }
    let Value::Object(map) = args else { return };

    // 1. timeFrom REPLACES the range (Grafana: the override wins over the dashboard range).
    if let Some(dur) = time_from {
        map.insert("from".into(), json!(now.saturating_sub(dur)));
        map.insert("to".into(), json!(now));
    }

    // 2. timeShift moves BOTH ends earlier — only over numeric epoch values (never another tool's
    // string vocabulary), and only when a range exists to shift.
    if let Some(shift) = time_shift {
        for key in ["from", "to"] {
            if let Some(n) = map.get(key).and_then(Value::as_u64) {
                map.insert(key.into(), json!(n.saturating_sub(shift)));
            }
        }
    }
}

/// Parse a Grafana-style duration string (`"30s"`, `"10m"`, `"6h"`, `"1d"`, `"2w"`, `"1M"`, `"1y"`)
/// to **milliseconds** — the unit of every `from`/`to` on this path. Fixed amounts (M = 30 d,
/// y = 365 d), matching Grafana's `rangeUtil` interval math — not calendar arithmetic. `None` on
/// empty/unparsable (the caller degrades to no-op), and on an overflow rather than a wrapped window.
fn parse_duration_ms(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num.parse().ok()?;
    let secs: u64 = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 7 * 86_400,
        "M" => 30 * 86_400,
        "y" => 365 * 86_400,
        _ => return None,
    };
    // A huge `n` (`"999999999999y"`) must degrade to a no-op like any other unusable duration,
    // never wrap into a small window that would look like a deliberate range.
    n.checked_mul(secs)?.checked_mul(1_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qo(time_from: &str, time_shift: &str) -> QueryOptions {
        QueryOptions {
            time_from: time_from.into(),
            time_shift: time_shift.into(),
            ..Default::default()
        }
    }

    /// `timeFrom` REPLACES the target's range with `[now - timeFrom, now]` — even over a
    /// caller-supplied range (Grafana: the panel override wins over the dashboard range).
    #[test]
    fn time_from_replaces_the_range() {
        // A realistic epoch-MS clock. (A tiny `now` would saturate at 0 rather than span 6h — see
        // `a_clock_smaller_than_the_override_saturates_at_the_epoch`.)
        let now: u64 = 1_751_414_400_000;
        let mut args = json!({ "series": "cooler.temp", "from": 1, "to": 2 });
        apply_time_override(&mut args, &qo("6h", ""), now);
        assert_eq!(args["from"], now - 6 * 3_600_000);
        assert_eq!(args["to"], now);
        assert_eq!(args["series"], "cooler.temp", "other args untouched");
    }

    /// The override is `saturating_sub`, so a clock EARLIER than the override window floors at the
    /// epoch rather than wrapping to a huge future instant. Only reachable with an injected test
    /// clock, but a wrap here would be an unbounded query rather than an empty one.
    #[test]
    fn a_clock_smaller_than_the_override_saturates_at_the_epoch() {
        let mut args = json!({});
        apply_time_override(&mut args, &qo("6h", ""), 1_000_000);
        assert_eq!(args["from"], 0);
        assert_eq!(args["to"], 1_000_000);
    }

    /// **The regression this file's unit bug would cause.** A realistic epoch-MS clock with
    /// `timeFrom: "6h"` must produce a six-hour window. The old seconds-valued parser produced a
    /// 21.6-SECOND window off the same inputs — 1000× too small, silently empty, never an error.
    #[test]
    fn time_from_is_milliseconds_against_a_real_epoch_ms_clock() {
        // 2025-07-02T00:00:00Z, the shape of clock the client actually sends.
        let now: u64 = 1_751_414_400_000;
        let mut args = json!({ "series": "meter.main" });
        apply_time_override(&mut args, &qo("6h", ""), now);

        let from = args["from"].as_u64().unwrap();
        let to = args["to"].as_u64().unwrap();
        assert_eq!(to, now);
        assert_eq!(
            to - from,
            6 * 60 * 60 * 1000,
            "a 6h override must span six hours of MILLISECONDS"
        );
    }

    /// `timeShift` moves BOTH ends earlier by the shift, over an existing numeric range.
    #[test]
    fn time_shift_moves_both_ends_earlier() {
        let mut args = json!({ "from": 10_000_000, "to": 20_000_000 });
        apply_time_override(&mut args, &qo("", "1h"), 99);
        assert_eq!(args["from"], 10_000_000 - 3_600_000);
        assert_eq!(args["to"], 20_000_000 - 3_600_000);
    }

    /// Combined: timeFrom sets `[now-6h, now]`, then timeShift moves that window back 1d.
    #[test]
    fn time_from_then_time_shift_compose() {
        let now: u64 = 1_751_414_400_000;
        let mut args = json!({});
        apply_time_override(&mut args, &qo("6h", "1d"), now);
        assert_eq!(args["from"], now - 6 * 3_600_000 - 86_400_000);
        assert_eq!(args["to"], now - 86_400_000);
    }

    /// A shift with NO existing range and no timeFrom is a no-op — nothing to shift; the host never
    /// invents a range the target didn't have.
    #[test]
    fn time_shift_without_a_range_is_a_noop() {
        let mut args = json!({ "series": "cooler.temp" });
        apply_time_override(&mut args, &qo("", "1h"), 99);
        assert_eq!(args, json!({ "series": "cooler.temp" }));
    }

    /// A NON-NUMERIC `from`/`to` (another tool's string vocabulary) is left untouched by timeShift —
    /// the host never guesses a vocabulary it doesn't own.
    #[test]
    fn non_numeric_range_left_untouched() {
        let mut args = json!({ "from": "now-1h", "to": "now" });
        apply_time_override(&mut args, &qo("", "1h"), 99);
        assert_eq!(args["from"], "now-1h");
        assert_eq!(args["to"], "now");
    }

    /// An unparsable duration degrades to a no-op (never a failed panel); `hideTimeOverride` alone
    /// never touches the query.
    #[test]
    fn unparsable_or_display_only_is_a_noop() {
        let mut args = json!({ "from": 1, "to": 2 });
        apply_time_override(&mut args, &qo("banana", ""), 99);
        assert_eq!(args, json!({ "from": 1, "to": 2 }));
        let display_only = QueryOptions {
            hide_time_override: true,
            ..Default::default()
        };
        apply_time_override(&mut args, &display_only, 99);
        assert_eq!(args, json!({ "from": 1, "to": 2 }));
    }

    /// The duration grammar: Grafana's fixed-amount units.
    #[test]
    fn duration_grammar() {
        assert_eq!(parse_duration_ms("30s"), Some(30_000));
        assert_eq!(parse_duration_ms("10m"), Some(600_000));
        assert_eq!(parse_duration_ms("2w"), Some(14 * 86_400_000));
        assert_eq!(parse_duration_ms("1M"), Some(30 * 86_400_000));
        assert_eq!(parse_duration_ms("1y"), Some(365 * 86_400_000));
        assert_eq!(parse_duration_ms(""), None);
        assert_eq!(parse_duration_ms("h"), None);
        assert_eq!(parse_duration_ms("1.5h"), None);
        // An absurd duration degrades to a no-op rather than wrapping into a small window.
        assert_eq!(parse_duration_ms("999999999999999y"), None);
    }
}
