//! `time` parsing + construction — string/parts → unix seconds. `parse` accepts the three shapes
//! sources actually hand back (RFC-3339/ISO-8601, epoch-secs-as-string, epoch-ms-as-string);
//! `parse_fmt` is strptime; `from_ymd`/`from_parts` build a UTC instant from components. Every
//! failure is a clear author error (never a silent 0).

use chrono::{DateTime, NaiveDate, NaiveDateTime};
use rhai::{Engine, EvalAltResult};

use super::TimeHandle;
use crate::grid::rhai_err;

pub(super) fn register(engine: &mut Engine) {
    engine.register_fn("parse", |_: &mut TimeHandle, s: &str| parse(s));
    engine.register_fn("parse_fmt", |_: &mut TimeHandle, s: &str, fmt: &str| {
        parse_fmt(s, fmt)
    });
    engine.register_fn("from_ymd", |_: &mut TimeHandle, y: i64, m: i64, d: i64| {
        from_parts(y, m, d, 0, 0, 0)
    });
    engine.register_fn(
        "from_parts",
        |_: &mut TimeHandle, y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64| {
            from_parts(y, m, d, h, mi, s)
        },
    );
}

/// Epoch-ms magnitudes start here (~year 33658 as seconds — every realistic epoch-secs value is
/// below it, every realistic epoch-ms value above). Same heuristic as `chart::normalize_epoch`.
const MS_THRESHOLD: i64 = 1_000_000_000_000;

/// RFC-3339/ISO-8601 (offset, `Z`, bare, or date-only; `T` or space separated), or a numeric
/// epoch (secs or ms) as a string → unix seconds.
fn parse(s: &str) -> Result<i64, Box<EvalAltResult>> {
    let s = s.trim();
    if let Ok(n) = s.parse::<i64>() {
        return Ok(if n.abs() >= MS_THRESHOLD { n / 1000 } else { n });
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.timestamp());
    }
    // ISO without an offset → UTC by contract; `%.f` tolerates optional fractional seconds.
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Ok(ndt.and_utc().timestamp());
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(midnight(d));
    }
    Err(rhai_err(format!(
        "cannot parse {s:?} as a timestamp (RFC-3339/ISO-8601, epoch secs, or epoch ms)"
    )))
}

/// strptime → unix seconds. Tries offset-aware, then naive-datetime (UTC), then date-only.
fn parse_fmt(s: &str, fmt: &str) -> Result<i64, Box<EvalAltResult>> {
    if let Ok(dt) = DateTime::parse_from_str(s, fmt) {
        return Ok(dt.timestamp());
    }
    if let Ok(ndt) = NaiveDateTime::parse_from_str(s, fmt) {
        return Ok(ndt.and_utc().timestamp());
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
        return Ok(midnight(d));
    }
    Err(rhai_err(format!("cannot parse {s:?} with format {fmt:?}")))
}

/// Build a UTC instant from calendar components; each component is range-checked (Feb 30 is an
/// author error, not a rollover).
fn from_parts(y: i64, m: i64, d: i64, h: i64, mi: i64, s: i64) -> Result<i64, Box<EvalAltResult>> {
    let bad = || {
        rhai_err(format!(
            "invalid date/time components {y:04}-{m:02}-{d:02} {h:02}:{mi:02}:{s:02}"
        ))
    };
    let date = NaiveDate::from_ymd_opt(
        i32::try_from(y).map_err(|_| bad())?,
        u32::try_from(m).map_err(|_| bad())?,
        u32::try_from(d).map_err(|_| bad())?,
    )
    .ok_or_else(bad)?;
    let dt = date
        .and_hms_opt(
            u32::try_from(h).map_err(|_| bad())?,
            u32::try_from(mi).map_err(|_| bad())?,
            u32::try_from(s).map_err(|_| bad())?,
        )
        .ok_or_else(bad)?;
    Ok(dt.and_utc().timestamp())
}

fn midnight(d: NaiveDate) -> i64 {
    // and_hms_opt(0,0,0) is always valid — midnight exists on every UTC day.
    d.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_609_459_200; // 2021-01-01T00:00:00Z

    #[test]
    fn parses_the_three_input_shapes() {
        assert_eq!(parse("2021-01-01T00:00:00Z").unwrap(), T0); // RFC-3339
        assert_eq!(parse("1609459200").unwrap(), T0); // epoch secs
        assert_eq!(parse("1609459200000").unwrap(), T0); // epoch ms
    }

    #[test]
    fn parses_iso_variants() {
        assert_eq!(parse("2021-01-01").unwrap(), T0); // date-only → midnight UTC
        assert_eq!(parse("2021-01-01 00:00:00").unwrap(), T0); // space separator
        assert_eq!(parse("2021-01-01T00:00:00.500").unwrap(), T0); // fraction truncates to secs
        assert_eq!(parse("2021-01-01T10:00:00+10:00").unwrap(), T0); // offset-aware
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(parse("not-a-date").is_err());
        assert!(parse("2021-13-01").is_err()); // bad month
    }

    #[test]
    fn parse_fmt_strptime() {
        assert_eq!(parse_fmt("01/01/2021", "%d/%m/%Y").unwrap(), T0);
        assert_eq!(
            parse_fmt("2021-01-01 00:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
            T0
        );
        assert!(parse_fmt("01/01/2021", "%Y-%m-%d").is_err());
    }

    #[test]
    fn from_parts_validates_the_calendar() {
        assert_eq!(from_parts(2021, 1, 1, 0, 0, 0).unwrap(), T0);
        assert_eq!(from_parts(2024, 2, 29, 0, 0, 0).is_ok(), true); // leap day exists
        assert!(from_parts(2023, 2, 29, 0, 0, 0).is_err()); // …only in a leap year
        assert!(from_parts(2021, 4, 31, 0, 0, 0).is_err()); // April has 30
        assert!(from_parts(2021, 1, 1, 24, 0, 0).is_err()); // hour 24
    }
}
