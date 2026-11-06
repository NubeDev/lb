//! The three text forms a schedule document uses: `HH:MM`, `YYYY-MM-DDTHH:MM` and day names.

use chrono::{NaiveDateTime, Weekday};

use crate::error::ScheduleError;

/// Minute of the day for `HH:MM` (`00:00` is 0, `23:59` is 1439). `24:00` is refused.
pub fn minutes_of(hhmm: &str) -> Result<u32, ScheduleError> {
    let bad = || ScheduleError::InvalidTimeFormat(hhmm.to_string());
    // Exactly two digits, a colon, two digits: `7:5` and `+7:05` are refused.
    let b = hhmm.as_bytes();
    if b.len() != 5 || b[2] != b':' || ![0, 1, 3, 4].iter().all(|&i| b[i].is_ascii_digit()) {
        return Err(bad());
    }
    let h: u32 = hhmm[..2].parse().map_err(|_| bad())?;
    let m: u32 = hhmm[3..].parse().map_err(|_| bad())?;
    if h > 23 || m > 59 {
        return Err(bad());
    }
    Ok(h * 60 + m)
}

/// A dated entry's local wall time, exactly as rubix-ce writes it (`YYYY-MM-DDTHH:MM`). Impossible
/// dates (`2026-02-30`), seconds, offsets and unpadded fields are refused.
pub fn parse_local(s: &str) -> Result<NaiveDateTime, ScheduleError> {
    const FORMAT: &str = "%Y-%m-%dT%H:%M";
    let invalid = |reason: String| ScheduleError::InvalidDatetime {
        value: s.to_string(),
        reason,
    };
    // chrono accepts signs, long years and unpadded fields, so the shape is checked first.
    let b = s.as_bytes();
    let shaped = b.len() == 16
        && b.iter().enumerate().all(|(i, c)| match i {
            4 | 7 => *c == b'-',
            10 => *c == b'T',
            13 => *c == b':',
            _ => c.is_ascii_digit(),
        });
    if !shaped {
        return Err(invalid("expected YYYY-MM-DDTHH:MM".to_string()));
    }
    NaiveDateTime::parse_from_str(s, FORMAT).map_err(|e| invalid(e.to_string()))
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
