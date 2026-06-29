//! `format.datetime(instant, opts?)` — render a stored **UTC instant** as wall-clock time in the
//! user's resolved timezone + date/time styles (prefs scope). The instant is epoch milliseconds
//! (the canonical wire form); the timezone is an IANA id resolved via `chrono-tz`, whose embedded
//! tz database carries the DST + historical rules (the scope's "tz over a UTC instant incl. a DST
//! boundary" — DST correctness is `chrono-tz`'s job, not ours).
//!
//! Date field order (`eu`/`iso`/`usa`) and 12h/24h come from the closed [`DateStyle`]/[`TimeStyle`]
//! axes — deterministic and locale-correct because the axes encode exactly those CLDR choices.
//! (Localized month *names* are a future icu path; the numeric styles the scope tests are exact.)

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};
use chrono_tz::Tz;

use crate::axis::{DateStyle, TimeStyle};
use crate::error::PrefsError;

/// Render `instant_ms` (epoch milliseconds, UTC) in `tz` with `date_style`/`time_style`. Returns
/// `"<date> <time>"`. Errors if the timezone id is not in the compiled database or the instant is
/// out of range.
pub fn format_datetime(
    instant_ms: i64,
    tz: &str,
    date_style: DateStyle,
    time_style: TimeStyle,
) -> Result<String, PrefsError> {
    let utc: DateTime<Utc> = Utc
        .timestamp_millis_opt(instant_ms)
        .single()
        .ok_or_else(|| PrefsError::BadInstant(format!("epoch ms out of range: {instant_ms}")))?;
    let zone: Tz = tz
        .parse()
        .map_err(|_| PrefsError::BadInstant(format!("unknown timezone: {tz}")))?;
    let local = utc.with_timezone(&zone);

    let date = render_date(local.year(), local.month(), local.day(), date_style);
    let time = render_time(local.hour(), local.minute(), time_style);
    Ok(format!("{date} {time}"))
}

fn render_date(year: i32, month: u32, day: u32, style: DateStyle) -> String {
    match style {
        DateStyle::Eu => format!("{day:02}/{month:02}/{year:04}"),
        DateStyle::Iso => format!("{year:04}-{month:02}-{day:02}"),
        DateStyle::Usa => format!("{month:02}/{day:02}/{year:04}"),
    }
}

fn render_time(hour24: u32, minute: u32, style: TimeStyle) -> String {
    match style {
        TimeStyle::H24 => format!("{hour24:02}:{minute:02}"),
        TimeStyle::H12 => {
            let (h12, meridiem) = match hour24 {
                0 => (12, "AM"),
                1..=11 => (hour24, "AM"),
                12 => (12, "PM"),
                _ => (hour24 - 12, "PM"),
            };
            format!("{h12}:{minute:02} {meridiem}")
        }
    }
}
