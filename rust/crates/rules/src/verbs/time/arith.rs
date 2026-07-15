//! `time` arithmetic — add/sub durations, floor/ceil bucket alignment, signed differences, and the
//! now-relative verbs (`since`/`until`/`ago`). Durations use the shared `s/m/h/d/w` parser in
//! `verbs::duration` (one grammar for `rollup`, `dur_*`, and here). Buckets are epoch-aligned —
//! the same alignment `rollup`'s `time::group` buckets use.

use rhai::{Engine, EvalAltResult};

use super::TimeHandle;
use crate::grid::rhai_err;
use crate::verbs::duration::{human_units, parse_secs};

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("add", |_: &mut TimeHandle, ts: i64, dur: &str| {
        shift(ts, dur, 1)
    });
    engine.register_fn("sub", |_: &mut TimeHandle, ts: i64, dur: &str| {
        shift(ts, dur, -1)
    });
    engine.register_fn("floor", |_: &mut TimeHandle, ts: i64, dur: &str| {
        let b = bucket(dur)?;
        Ok::<i64, Box<EvalAltResult>>(ts.div_euclid(b) * b)
    });
    engine.register_fn("ceil", |_: &mut TimeHandle, ts: i64, dur: &str| {
        ceil(ts, dur)
    });
    engine.register_fn("diff", |_: &mut TimeHandle, a: i64, b: i64| a - b);
    engine.register_fn("diff_days", |_: &mut TimeHandle, a: i64, b: i64| {
        (a - b) / 86_400
    });
    engine.register_fn("since", |t: &mut TimeHandle, ts: i64| t.now_secs() - ts);
    engine.register_fn("until", |t: &mut TimeHandle, ts: i64| ts - t.now_secs());
    engine.register_fn("ago", |t: &mut TimeHandle, ts: i64| ago(t.now_secs() - ts));
}

fn shift(ts: i64, dur: &str, sign: i64) -> Result<i64, Box<EvalAltResult>> {
    let secs = parse_secs(dur).map_err(rhai_err)?;
    ts.checked_add(sign * secs)
        .ok_or_else(|| rhai_err(format!("timestamp arithmetic overflows ({ts} ± {dur})")))
}

/// A bucket width for floor/ceil — parsed and required positive (`"0m"` would divide by zero).
fn bucket(dur: &str) -> Result<i64, Box<EvalAltResult>> {
    let b = parse_secs(dur).map_err(rhai_err)?;
    if b <= 0 {
        return Err(rhai_err(format!("bucket {dur:?} must be positive")));
    }
    Ok(b)
}

/// Smallest bucket boundary ≥ ts (a ts already on the boundary stays put).
fn ceil(ts: i64, dur: &str) -> Result<i64, Box<EvalAltResult>> {
    let b = bucket(dur)?;
    let floored = ts.div_euclid(b) * b;
    if floored == ts {
        Ok(ts)
    } else {
        floored
            .checked_add(b)
            .ok_or_else(|| rhai_err(format!("timestamp {ts} ceil {dur:?} overflows")))
    }
}

/// Humanize a now-relative delta for message bodies: positive → past (`"3h 20m ago"`), negative →
/// future (`"in 3h 20m"`), zero → `"just now"`. Two most-significant units keep it readable.
fn ago(delta: i64) -> String {
    if delta == 0 {
        return "just now".to_string();
    }
    let phrase = human_units(delta.abs(), 2);
    if delta > 0 {
        format!("{phrase} ago")
    } else {
        format!("in {phrase}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_sub_shift_by_the_duration() {
        assert_eq!(shift(1000, "90m", 1).unwrap(), 1000 + 5400);
        assert_eq!(shift(1000, "7d", -1).unwrap(), 1000 - 7 * 86_400);
        assert!(shift(i64::MAX, "1w", 1).is_err()); // overflow is an error, not a wrap
        assert!(shift(0, "10y", 1).is_err()); // unknown unit surfaces the parser's message
    }

    #[test]
    fn floor_ceil_bucket_alignment() {
        // 15m = 900s buckets, epoch-aligned.
        assert_eq!(1000_i64.div_euclid(900) * 900, 900);
        assert_eq!(ceil(1000, "15m").unwrap(), 1800);
        assert_eq!(ceil(1800, "15m").unwrap(), 1800); // on-boundary stays put
        assert_eq!((-100_i64).div_euclid(900) * 900, -900); // pre-epoch floors DOWN
        assert!(ceil(1000, "0m").is_err()); // zero bucket refused
    }

    #[test]
    fn ago_phrasing() {
        assert_eq!(ago(3 * 3600 + 20 * 60), "3h 20m ago");
        assert_eq!(ago(-(3 * 3600 + 20 * 60)), "in 3h 20m");
        assert_eq!(ago(45), "45s ago");
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(8 * 86_400 + 7200), "1w 1d ago"); // two most-significant units only
    }
}
