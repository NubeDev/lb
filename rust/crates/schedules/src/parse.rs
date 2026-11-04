//! The three text forms a schedule document uses: `HH:MM`, `YYYY-MM-DDTHH:MM` and day names.

use chrono::{NaiveDateTime, Weekday};

use crate::error::ScheduleError;

/// Minute of the day for `HH:MM` (`00:00` is 0, `23:59` is 1439). `24:00` is refused.
pub fn minutes_of(hhmm: &str) -> Result<u32, ScheduleError> {
    let bad = || ScheduleError::InvalidTimeFormat(hhmm.to_string());
    let (h, m) = hhmm.split_once(':').ok_or_else(bad)?;
    let h: u32 = h.parse().map_err(|_| bad())?;
    let m: u32 = m.parse().map_err(|_| bad())?;
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// A dated entry's local wall time, exactly as rubix-ce writes it (`YYYY-MM-DDTHH:MM`). Impossible
/// dates (`2026-02-30`) are refused.
pub fn parse_local(s: &str) -> Result<NaiveDateTime, ScheduleError> {
    NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M").map_err(|e| ScheduleError::InvalidDatetime {
        value: s.to_string(),
        reason: e.to_string(),
    })
}

/// rubix-ce's lowercase day names.
pub fn weekday_from_name(name: &str) -> Option<Weekday> {
    Some(match name {
        "monday" => Weekday::Mon,
        "tuesday" => Weekday::Tue,
        "wednesday" => Weekday::Wed,
        "thursday" => Weekday::Thu,
        "friday" => Weekday::Fri,
        "saturday" => Weekday::Sat,
        "sunday" => Weekday::Sun,
        _ => return None,
    })
}
