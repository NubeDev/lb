//! `time` formatting — timestamp → string. UTC always, except the explicit fixed-offset arity of
//! `format` (`"+HH:MM"`/`"-HH:MM"` — no tz database in v1, per the scope's non-goals). Format
//! strings are validated up front: chrono's `DelayedFormat` PANICS on a bad specifier at write
//! time, so we pre-scan the strftime items and return an author error instead.

use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use rhai::{Engine, EvalAltResult};

use super::{utc, TimeHandle};
use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("iso", |_: &mut TimeHandle, ts: i64| iso(ts));
    engine.register_fn("iso_ms", |_: &mut TimeHandle, ts_ms: i64| iso_ms(ts_ms));
    engine.register_fn("date", |_: &mut TimeHandle, ts: i64| {
        Ok::<_, Box<EvalAltResult>>(utc(ts)?.format("%Y-%m-%d").to_string())
    });
    engine.register_fn("clock", |_: &mut TimeHandle, ts: i64| {
        Ok::<_, Box<EvalAltResult>>(utc(ts)?.format("%H:%M:%S").to_string())
    });
    engine.register_fn("format", |_: &mut TimeHandle, ts: i64, fmt: &str| {
        strftime(&utc(ts)?, fmt)
    });
    engine.register_fn(
        "format",
        |_: &mut TimeHandle, ts: i64, fmt: &str, offset: &str| {
            let off = parse_offset(offset)?;
            strftime(&utc(ts)?.with_timezone(&off), fmt)
        },
    );
}

/// RFC-3339 instant, second precision: `"2026-07-04T03:21:00Z"`.
fn iso(ts: i64) -> Result<String, Box<EvalAltResult>> {
    Ok(utc(ts)?.format("%Y-%m-%dT%H:%M:%SZ").to_string())
}

/// RFC-3339 instant with milliseconds: `"2026-07-04T03:21:00.250Z"` (the `_ms` variant keeps the
/// sub-second precision its input carries).
fn iso_ms(ts_ms: i64) -> Result<String, Box<EvalAltResult>> {
    let dt = DateTime::<Utc>::from_timestamp_millis(ts_ms)
        .ok_or_else(|| rhai_err(format!("timestamp {ts_ms}ms is out of range")))?;
    Ok(dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
}

/// strftime with the format string validated first (a bad specifier is author feedback, never a
/// panic inside the cage).
fn strftime<Tz: TimeZone>(dt: &DateTime<Tz>, fmt: &str) -> Result<String, Box<EvalAltResult>>
where
    Tz::Offset: std::fmt::Display,
{
    let items: Vec<Item> = StrftimeItems::new(fmt).collect();
    if items.iter().any(|i| matches!(i, Item::Error)) {
        return Err(rhai_err(format!("invalid strftime format {fmt:?}")));
    }
    Ok(dt.format_with_items(items.into_iter()).to_string())
}

/// Parse `"+HH:MM"` / `"-HH:MM"` into a fixed offset. The only timezone form v1 accepts.
fn parse_offset(s: &str) -> Result<FixedOffset, Box<EvalAltResult>> {
    let err = || rhai_err(format!("offset {s:?} must be \"+HH:MM\" or \"-HH:MM\""));
    let (sign, rest) = match (s.strip_prefix('+'), s.strip_prefix('-')) {
        (Some(r), _) => (1, r),
        (_, Some(r)) => (-1, r),
        _ => return Err(err()),
    };
    let (hh, mm) = rest.split_once(':').ok_or_else(err)?;
    if hh.len() != 2 || mm.len() != 2 {
        return Err(err());
    }
    let h: i32 = hh.parse().map_err(|_| err())?;
    let m: i32 = mm.parse().map_err(|_| err())?;
    if h > 23 || m > 59 {
        return Err(err());
    }
    FixedOffset::east_opt(sign * (h * 3600 + m * 60)).ok_or_else(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2021-01-01T00:00:00Z (18628 days — pinned by chart.rs's civil-days test too).
    const T0: i64 = 1_609_459_200;

    #[test]
    fn iso_and_parts() {
        assert_eq!(iso(T0).unwrap(), "2021-01-01T00:00:00Z");
        assert_eq!(iso(T0 + 3661).unwrap(), "2021-01-01T01:01:01Z");
        assert_eq!(iso_ms(T0 * 1000 + 250).unwrap(), "2021-01-01T00:00:00.250Z");
    }

    #[test]
    fn fixed_offset_formats() {
        let east = parse_offset("+10:00").unwrap();
        let s = strftime(&utc(T0).unwrap().with_timezone(&east), "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(s, "2021-01-01 10:00");
        let west = parse_offset("-05:30").unwrap();
        let s = strftime(&utc(T0).unwrap().with_timezone(&west), "%Y-%m-%d %H:%M").unwrap();
        assert_eq!(s, "2020-12-31 18:30");
    }

    #[test]
    fn bad_offset_and_bad_fmt_are_author_errors() {
        assert!(parse_offset("10:00").is_err());
        assert!(parse_offset("+25:00").is_err());
        assert!(parse_offset("+1:0").is_err());
        // `%Q` is not a strftime specifier — must error, never panic.
        assert!(strftime(&utc(T0).unwrap(), "%Q").is_err());
    }
}
