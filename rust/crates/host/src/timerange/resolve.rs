//! `resolve` — one `(from, to?, now, tz)` → a concrete `[from_ms, to_ms)` window plus its ISO-day
//! projection (the day-granular form the URL/report path speaks). Pure arithmetic over an injected
//! clock — no wall-time is read here (symmetric nodes: the clock is a parameter).
//!
//! Decided semantics (the scope's "The grammar"): `to` is **exclusive**; a range token is legal
//! only in `from` with `to` absent; an endpoint `from` with `to` absent ends at `now`;
//! month/year stepping is calendar-aware and clamped (31 Mar − 1 month = 28 Feb); weeks start
//! Monday; quarters are Jan/Apr/Jul/Oct; snaps floor to the start of their unit in the range tz.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Timelike};
use chrono_tz::Tz;

use super::civil::{
    add_months, civil_from_days, days_from_civil, iso_day, quarter_start_month, weekday,
};
use super::grammar::{
    parse_expr, CalUnit, Endpoint, EndpointBase, RangeExpr, TimeRangeError, Unit, Window,
};

/// A resolved window: the ms pair (`to` exclusive) plus the ISO-day projection — the smallest
/// day-granular window containing `[from_ms, to_ms)` in the range tz, `to_day` exclusive (a
/// midnight `to` names its own day; anything later rounds up).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ResolvedRange {
    pub from_ms: i64,
    pub to_ms: i64,
    /// ISO `yyyy-mm-dd` of the day containing `from_ms` (in the range tz).
    pub from_day: String,
    /// ISO `yyyy-mm-dd` exclusive end day: the first day NOT in the window.
    pub to_day: String,
}

/// Parse an IANA timezone name. Empty (and `"UTC"`) is UTC; an unknown name is a normal refusal
/// naming the bad value — never a panic (the string-in form the embedder seam needs).
pub fn parse_tz(tz: &str) -> Result<Tz, TimeRangeError> {
    let tz = tz.trim();
    if tz.is_empty() {
        return Ok(Tz::UTC);
    }
    tz.parse::<Tz>()
        .map_err(|_| TimeRangeError::BadTimezone { tz: tz.to_string() })
}

/// [`resolve`] with the timezone as a **string** IANA name — the embedder-facing form (a downstream
/// host with no chrono-tz dep can call this; `lb-node` re-exports it).
pub fn resolve_range(
    from: &str,
    to: Option<&str>,
    now_ms: i64,
    tz: &str,
) -> Result<ResolvedRange, TimeRangeError> {
    resolve(from, to, now_ms, parse_tz(tz)?)
}

/// Resolve `(from, to?)` against `now_ms` in `tz`. A range token in `from` requires `to` absent
/// (it IS both ends); an endpoint `from` with no `to` ends at `now`.
pub fn resolve(
    from: &str,
    to: Option<&str>,
    now_ms: i64,
    tz: Tz,
) -> Result<ResolvedRange, TimeRangeError> {
    let from_expr = parse_expr("from", from)?;
    let (from_ms, to_ms) = match from_expr {
        RangeExpr::Window(w) => {
            if let Some(t) = to {
                if !t.trim().is_empty() {
                    return Err(TimeRangeError::WindowWithTo {
                        token: from.trim().to_string(),
                    });
                }
            }
            window_span(w, now_ms, tz)
        }
        RangeExpr::Endpoint(e) => {
            let f = endpoint_ms(&e, now_ms, tz);
            let t = match to.map(str::trim).filter(|t| !t.is_empty()) {
                None => now_ms,
                Some(t) => match parse_expr("to", t)? {
                    RangeExpr::Window(_) => {
                        return Err(TimeRangeError::WindowInTo {
                            token: t.to_string(),
                        })
                    }
                    RangeExpr::Endpoint(e2) => endpoint_ms(&e2, now_ms, tz),
                },
            };
            (f, t)
        }
    };
    if to_ms < from_ms {
        return Err(TimeRangeError::Inverted {
            from: from.trim().to_string(),
            to: to.map(str::trim).unwrap_or("now").to_string(),
        });
    }
    Ok(ResolvedRange {
        from_ms,
        to_ms,
        from_day: day_of(from_ms, tz, false),
        to_day: day_of(to_ms, tz, true),
    })
}

/// Validate a `(from, to?)` pair structurally — a fixed clock, UTC. What `dashboard.save` and the
/// schedule-payload save run: a bad expression fails HERE with a human watching, never at 03:00.
pub fn validate(from: &str, to: Option<&str>) -> Result<(), TimeRangeError> {
    // Any valid instant works — resolution is total once the parse and shape checks pass.
    const VALIDATE_NOW_MS: i64 = 1_785_283_200_000; // 2026-07-29T00:00:00Z
    resolve(from, to, VALIDATE_NOW_MS, Tz::UTC).map(|_| ())
}

// ── instant ↔ local civil time ──────────────────────────────────────────────────────────────────

/// The local civil datetime of an instant in `tz`.
fn local_of(ms: i64, tz: Tz) -> NaiveDateTime {
    tz.timestamp_millis_opt(ms)
        .earliest()
        .map(|dt| dt.naive_local())
        .unwrap_or_default()
}

/// The instant of a local civil datetime in `tz`, DST-disambiguated: an ambiguous local time
/// (fall-back overlap) takes the EARLIER instant; a nonexistent one (spring-forward gap) shifts
/// forward an hour — the window still covers the real elapsed time, never errors.
fn ms_of_local(naive: NaiveDateTime, tz: Tz) -> i64 {
    match tz.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt.timestamp_millis(),
        chrono::LocalResult::Ambiguous(a, _) => a.timestamp_millis(),
        chrono::LocalResult::None => {
            let shifted = naive + Duration::hours(1);
            tz.from_local_datetime(&shifted)
                .earliest()
                .map(|dt| dt.timestamp_millis())
                .unwrap_or_default()
        }
    }
}

/// Split a local datetime into (days since epoch, ms since local midnight) — the civil coordinates
/// [`super::civil`] steps in.
fn split(naive: NaiveDateTime) -> (i64, i64) {
    let d = naive.date();
    let days = days_from_civil(d.year() as i64, d.month(), d.day());
    let t = naive.time();
    let msod = t.num_seconds_from_midnight() as i64 * 1000 + (t.nanosecond() / 1_000_000) as i64;
    (days, msod)
}

/// The inverse of [`split`].
fn join(days: i64, msod: i64) -> NaiveDateTime {
    let (y, m, d) = civil_from_days(days);
    let date = NaiveDate::from_ymd_opt(y as i32, m, d).unwrap_or_default();
    let time = NaiveTime::from_num_seconds_from_midnight_opt(
        (msod / 1000) as u32,
        ((msod % 1000) * 1_000_000) as u32,
    )
    .unwrap_or_default();
    date.and_time(time)
}

// ── endpoint resolution ─────────────────────────────────────────────────────────────────────────

/// Resolve one endpoint to an instant.
fn endpoint_ms(e: &Endpoint, now_ms: i64, tz: Tz) -> i64 {
    let mut ms = match &e.base {
        EndpointBase::Now { offset } => match offset {
            None => now_ms,
            Some((n, unit)) => offset_ms(now_ms, *n, *unit, tz),
        },
        EndpointBase::IsoDay(d) => {
            let days = days_from_civil(d.year() as i64, d.month(), d.day());
            ms_of_local(join(days, 0), tz)
        }
        EndpointBase::InstantFixed(ms) => *ms,
        EndpointBase::InstantLocal(naive) => ms_of_local(*naive, tz),
        EndpointBase::EpochMs(ms) => *ms,
    };
    if let Some(unit) = e.snap {
        ms = snap_floor(ms, unit, tz);
    }
    ms
}

/// Apply a signed unit offset to an instant. Sub-day units are absolute durations; day and larger
/// step the LOCAL calendar preserving the wall-clock time (calendar-aware, clamped months/years —
/// the scope's decided semantics; `now-1d` across a DST change lands on the same wall-clock time).
fn offset_ms(ms: i64, n: i64, unit: Unit, tz: Tz) -> i64 {
    match unit {
        Unit::Second => ms + n * 1_000,
        Unit::Minute => ms + n * 60_000,
        Unit::Hour => ms + n * 3_600_000,
        Unit::Day => step_days(ms, n, tz),
        Unit::Week => step_days(ms, n * 7, tz),
        Unit::Month => step_months(ms, n, tz),
        Unit::Year => step_months(ms, n * 12, tz),
    }
}

/// Step whole local days, keeping the wall-clock time.
fn step_days(ms: i64, n: i64, tz: Tz) -> i64 {
    let (days, msod) = split(local_of(ms, tz));
    ms_of_local(join(days + n, msod), tz)
}

/// Step whole local calendar months (clamped day), keeping the wall-clock time.
fn step_months(ms: i64, n: i64, tz: Tz) -> i64 {
    let (days, msod) = split(local_of(ms, tz));
    let (y, m, d) = civil_from_days(days);
    let (ny, nm, nd) = add_months(y, m, d, n);
    ms_of_local(join(days_from_civil(ny, nm, nd), msod), tz)
}

/// Floor an instant to the start of `unit` in local time (weeks floor to Monday).
fn snap_floor(ms: i64, unit: Unit, tz: Tz) -> i64 {
    let (days, msod) = split(local_of(ms, tz));
    let (days, msod) = match unit {
        Unit::Second => (days, msod - msod % 1_000),
        Unit::Minute => (days, msod - msod % 60_000),
        Unit::Hour => (days, msod - msod % 3_600_000),
        Unit::Day => (days, 0),
        Unit::Week => (days - weekday(days) as i64, 0),
        Unit::Month => {
            let (y, m, _) = civil_from_days(days);
            (days_from_civil(y, m, 1), 0)
        }
        Unit::Year => {
            let (y, _, _) = civil_from_days(days);
            (days_from_civil(y, 1, 1), 0)
        }
    };
    ms_of_local(join(days, msod), tz)
}

// ── window resolution ───────────────────────────────────────────────────────────────────────────

/// Resolve a range token to its whole `[from, to)` window in `tz`.
fn window_span(w: Window, now_ms: i64, tz: Tz) -> (i64, i64) {
    let (d0, msod) = split(local_of(now_ms, tz));
    let day = |d: i64| ms_of_local(join(d, 0), tz);
    match w {
        Window::Today => (day(d0), day(d0 + 1)),
        Window::Yesterday => (day(d0 - 1), day(d0)),
        Window::Tomorrow => (day(d0 + 1), day(d0 + 2)),
        Window::This(u) => cal_span(u, d0, msod, 0, tz),
        Window::LastCal(u) => cal_span(u, d0, msod, -1, tz),
        Window::Next(u) => cal_span(u, d0, msod, 1, tz),
        // A trailing window ends NOW; its start is the same clamped calendar step an endpoint
        // offset takes (last-1-month on 31 Mar starts 28 Feb).
        Window::Trailing { n, unit } => (offset_ms(now_ms, -(n as i64), unit, tz), now_ms),
    }
}

/// The whole calendar period `shift` periods away from the one containing now (`shift` −1 = the
/// previous whole period, 0 = this one, +1 = the next).
fn cal_span(unit: CalUnit, d0: i64, msod: i64, shift: i64, tz: Tz) -> (i64, i64) {
    let day = |d: i64| ms_of_local(join(d, 0), tz);
    match unit {
        CalUnit::Hour => {
            let start = join(d0, msod - msod % 3_600_000) + Duration::hours(shift);
            (
                ms_of_local(start, tz),
                ms_of_local(start + Duration::hours(1), tz),
            )
        }
        CalUnit::Day => (day(d0 + shift), day(d0 + shift + 1)),
        CalUnit::Week => {
            let monday = d0 - weekday(d0) as i64 + 7 * shift;
            (day(monday), day(monday + 7))
        }
        CalUnit::Month => month_span(d0, shift, 1, day),
        CalUnit::Quarter => {
            let (y, m, _) = civil_from_days(d0);
            let qd = days_from_civil(y, quarter_start_month(m), 1);
            month_span(qd, shift, 3, day)
        }
        CalUnit::Year => {
            let (y, _, _) = civil_from_days(d0);
            month_span(days_from_civil(y, 1, 1), shift, 12, day)
        }
    }
}

/// A whole `width`-month period `shift` periods from the one starting at the month containing
/// `anchor_days` — months, quarters and years share this one stepping.
fn month_span(anchor_days: i64, shift: i64, width: i64, day: impl Fn(i64) -> i64) -> (i64, i64) {
    let (y, m, _) = civil_from_days(anchor_days);
    let (fy, fm, _) = add_months(y, m, 1, shift * width);
    let (ty, tm, _) = add_months(fy, fm, 1, width);
    (
        day(days_from_civil(fy, fm, 1)),
        day(days_from_civil(ty, tm, 1)),
    )
}

/// The ISO-day projection of one instant: the local day containing it, or (for an exclusive `to`)
/// the first day NOT in the window — a midnight `to` names its own day, anything later rounds up.
fn day_of(ms: i64, tz: Tz, exclusive_end: bool) -> String {
    let (days, msod) = split(local_of(ms, tz));
    iso_day(if exclusive_end && msod > 0 {
        days + 1
    } else {
        days
    })
}
