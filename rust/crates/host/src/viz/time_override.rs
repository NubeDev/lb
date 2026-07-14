//! Panel time override for `viz.query` target dispatch (viz grafana-parity-backend scope, P1).
//! Grafana's `applyPanelTimeOverrides` semantics, pinned in the P1 session doc:
//!
//!  1. `timeFrom` **replaces** the range: `[now - timeFrom, now]` — an override, not a nudge.
//!  2. `timeShift` then moves **both** ends earlier by the shift (`from -= shift, to -= shift`).
//!  3. `hideTimeOverride` is display-only — it never touches the query.
//!
//! Bounded on purpose: the host applies the override only to a target's **numeric epoch-second**
//! `from`/`to` args (the `series.read` contract — the one range vocabulary the platform has).
//! A target whose args carry no range and no `timeFrom` is left alone (`timeShift` has nothing to
//! shift); a non-numeric `from`/`to` (a string expression some ext tool owns) is left untouched —
//! the host never guesses another tool's vocabulary. An unparsable duration is a silent no-op
//! (degrade, never a failed panel). Pure — no store, no clock (the caller threads logical `now`).

use serde_json::{json, Value};

use crate::dashboard::QueryOptions;

/// Apply a panel's `timeFrom`/`timeShift` to one target's `args` before dispatch. `now` is the
/// caller's logical clock (epoch seconds), the same one threaded as `ts`.
pub fn apply_time_override(args: &mut Value, qo: &QueryOptions, now: u64) {
    let time_from = parse_duration_secs(&qo.time_from);
    let time_shift = parse_duration_secs(&qo.time_shift);
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
/// to seconds. Fixed amounts (M = 30 d, y = 365 d), matching Grafana's `rangeUtil` interval math —
/// not calendar arithmetic. `None` on empty/unparsable (the caller degrades to no-op).
fn parse_duration_secs(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num.parse().ok()?;
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 7 * 86_400,
        "M" => 30 * 86_400,
        "y" => 365 * 86_400,
        _ => return None,
    };
    Some(n * mult)
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
        let now = 1_000_000;
        let mut args = json!({ "series": "cooler.temp", "from": 1, "to": 2 });
        apply_time_override(&mut args, &qo("6h", ""), now);
        assert_eq!(args["from"], now - 6 * 3600);
        assert_eq!(args["to"], now);
        assert_eq!(args["series"], "cooler.temp", "other args untouched");
    }

    /// `timeShift` moves BOTH ends earlier by the shift, over an existing numeric range.
    #[test]
    fn time_shift_moves_both_ends_earlier() {
        let mut args = json!({ "from": 10_000, "to": 20_000 });
        apply_time_override(&mut args, &qo("", "1h"), 99);
        assert_eq!(args["from"], 10_000 - 3600);
        assert_eq!(args["to"], 20_000 - 3600);
    }

    /// Combined: timeFrom sets `[now-6h, now]`, then timeShift moves that window back 1d.
    #[test]
    fn time_from_then_time_shift_compose() {
        let now = 1_000_000;
        let mut args = json!({});
        apply_time_override(&mut args, &qo("6h", "1d"), now);
        assert_eq!(args["from"], now - 6 * 3600 - 86_400);
        assert_eq!(args["to"], now - 86_400);
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
        assert_eq!(parse_duration_secs("30s"), Some(30));
        assert_eq!(parse_duration_secs("10m"), Some(600));
        assert_eq!(parse_duration_secs("2w"), Some(14 * 86_400));
        assert_eq!(parse_duration_secs("1M"), Some(30 * 86_400));
        assert_eq!(parse_duration_secs("1y"), Some(365 * 86_400));
        assert_eq!(parse_duration_secs(""), None);
        assert_eq!(parse_duration_secs("h"), None);
        assert_eq!(parse_duration_secs("1.5h"), None);
    }
}
