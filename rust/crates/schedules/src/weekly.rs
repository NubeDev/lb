use chrono::{DateTime, Datelike, Duration, TimeZone, Utc, Weekday};

use crate::error::ScheduleError;
use crate::schedule::Schedule;
use crate::types::{ScheduleStatus, TimeRange};

impl Schedule {
    /// Add a time-of-day range to a specific weekday.
    ///
    /// `start` and `stop` must be in `HH:MM` format. A stop time earlier than
    /// start (e.g. `"22:00"` → `"02:00"`) is automatically flagged as
    /// `spans_midnight`.
    pub fn add_weekly_time_range(
        &mut self,
        day: Weekday,
        start: &str,
        stop: &str,
    ) -> Result<(), ScheduleError> {
        let (h1, m1) = Self::parse_hhmm(start)?;
        let (h2, m2) = Self::parse_hhmm(stop)?;

        let spans_midnight = h2 < h1 || (h2 == h1 && m2 < m1);

        let tr = TimeRange {
            start: start.to_string(),
            stop: stop.to_string(),
            spans_midnight,
        };

        self.weekly.entry(day).or_default().push(tr);
        Ok(())
    }

    /// Evaluate all weekly entries and return their current status.
    pub fn check_weekly(&self) -> Vec<ScheduleStatus> {
        let now = self.now();
        let mut out = Vec::new();

        for (day, ranges) in &self.weekly {
            for tr in ranges {
                out.extend(self.evaluate_weekly_range(*day, tr, now));
            }
        }

        out
    }

    /// Returns `true` when at least one weekly entry is currently active.
    pub fn weekly_active(&self) -> bool {
        self.check_weekly().iter().any(|s| s.is_active)
    }

    // --- internals ---

    fn evaluate_weekly_range(
        &self,
        day: Weekday,
        tr: &TimeRange,
        now: DateTime<Utc>,
    ) -> Vec<ScheduleStatus> {
        let (sh, sm) = Self::parse_hhmm(&tr.start).unwrap();
        let (eh, em) = Self::parse_hhmm(&tr.stop).unwrap();

        if !tr.spans_midnight {
            self.eval_same_day(day, sh, sm, eh, em, now)
        } else {
            self.eval_midnight_span(day, sh, sm, eh, em, now)
        }
    }

    /// Evaluate a window that starts and ends on the same weekday.
    fn eval_same_day(
        &self,
        day: Weekday,
        sh: u32,
        sm: u32,
        eh: u32,
        em: u32,
        now: DateTime<Utc>,
    ) -> Vec<ScheduleStatus> {
        let week = Duration::weeks(1);
        let mut start = self.time_on_weekday(day, sh, sm, now);
        let mut stop = self.time_on_weekday(day, eh, em, now);

        // Advance until stop is in the future
        while stop <= now {
            start += week;
            stop += week;
        }

        let is_active = now > start && now < stop;
        let (next_start, next_stop) = if is_active || now >= stop {
            (start + week, stop + week)
        } else {
            (start, stop)
        };

        vec![ScheduleStatus {
            is_active,
            start_date: start,
            end_date: stop,
            next_start: Some(next_start),
            next_stop: Some(next_stop),
            source: "weekly".to_string(),
            priority: self.priority,
        }]
    }

    /// Evaluate a window that crosses midnight (e.g. 22:00 → 02:00).
    fn eval_midnight_span(
        &self,
        day: Weekday,
        sh: u32,
        sm: u32,
        eh: u32,
        em: u32,
        now: DateTime<Utc>,
    ) -> Vec<ScheduleStatus> {
        let week = Duration::weeks(1);
        let next_day = next_weekday(day);

        let mut start = self.time_on_weekday(day, sh, sm, now);
        let mut stop = self.time_on_weekday(next_day, eh, em, now);

        // Ensure stop is after start
        if stop <= start {
            stop += week;
        }

        // Advance until stop is in the future
        while stop <= now {
            start += week;
            stop += week;
        }

        let is_active = now > start && now < stop;
        let (next_start, next_stop) = if is_active || now >= stop {
            (start + week, stop + week)
        } else {
            (start, stop)
        };

        vec![ScheduleStatus {
            is_active,
            start_date: start,
            end_date: stop,
            next_start: Some(next_start),
            next_stop: Some(next_stop),
            source: "weekly-midnight-span".to_string(),
            priority: self.priority,
        }]
    }

    /// Compute a `DateTime<Utc>` for a given weekday + time-of-day relative to `reference`.
    ///
    /// When a non-UTC timezone is configured the time is constructed in that
    /// timezone so that DST offsets are respected, then converted back to UTC.
    pub(crate) fn time_on_weekday(
        &self,
        day: Weekday,
        hour: u32,
        minute: u32,
        reference: DateTime<Utc>,
    ) -> DateTime<Utc> {
        // The reference weekday AND calendar date must be read in the SCHEDULE's timezone, not in
        // UTC. Reading them in UTC while building the time-of-day in `tz` (below) puts the two on
        // different calendar days whenever the zone's offset has pushed local time across midnight:
        // Monday 08:00 in Australia/Brisbane (UTC+10) is Sunday 22:00 UTC, so a UTC-derived weekday
        // says "Sunday" and a Monday window resolves a day late. Anchor both in the same zone.
        let (ref_weekday, ref_date) = match self.tz {
            Some(tz) => {
                let local = reference.with_timezone(&tz);
                (local.weekday(), local.date_naive())
            }
            None => (reference.weekday(), reference.date_naive()),
        };

        let days_from_mon_ref = ref_weekday.num_days_from_monday() as i64;
        let days_from_mon_target = day.num_days_from_monday() as i64;
        let mut delta = days_from_mon_target - days_from_mon_ref;
        if delta < 0 {
            delta += 7;
        }

        let base_date = ref_date + chrono::Days::new(delta as u64);

        if let Some(tz) = self.tz {
            // Build time in local tz so DST is handled correctly
            let local = tz
                .with_ymd_and_hms(
                    base_date.year(),
                    base_date.month(),
                    base_date.day(),
                    hour,
                    minute,
                    0,
                )
                .single()
                .unwrap_or_else(|| {
                    // Ambiguous or non-existent time (DST gap) — fall back to UTC offset
                    tz.with_ymd_and_hms(
                        base_date.year(),
                        base_date.month(),
                        base_date.day(),
                        hour,
                        minute,
                        0,
                    )
                    .earliest()
                    .expect("datetime construction failed")
                });
            local.with_timezone(&Utc)
        } else {
            // UTC or local system time
            Utc.with_ymd_and_hms(
                base_date.year(),
                base_date.month(),
                base_date.day(),
                hour,
                minute,
                0,
            )
            .single()
            .expect("utc datetime construction failed")
        }
    }
}

fn next_weekday(day: Weekday) -> Weekday {
    match day {
        Weekday::Mon => Weekday::Tue,
        Weekday::Tue => Weekday::Wed,
        Weekday::Wed => Weekday::Thu,
        Weekday::Thu => Weekday::Fri,
        Weekday::Fri => Weekday::Sat,
        Weekday::Sat => Weekday::Sun,
        Weekday::Sun => Weekday::Mon,
    }
}

#[cfg(test)]
mod tests {
    use crate::Schedule;
    use chrono::{TimeZone, Utc, Weekday};

    /// **The timezone weekday regression.** The reference weekday/date must be read in the
    /// schedule's own zone. Monday 08:00 in Australia/Brisbane (UTC+10) is *Sunday* 22:00 UTC;
    /// deriving the weekday from UTC resolved a Monday window onto the wrong calendar day, so a
    /// 09:00–17:00 Monday schedule reported its next start a full day late.
    #[test]
    fn weekday_is_anchored_in_the_schedule_timezone() {
        let mut s = Schedule::new("bne", false, "Australia/Brisbane", 1).unwrap();
        s.add_weekly_time_range(Weekday::Mon, "09:00", "17:00")
            .unwrap();

        // Monday 2026-08-03 08:00 Brisbane == Sunday 2026-08-02 22:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 8, 2, 22, 0, 0).unwrap();
        let start = s.time_on_weekday(Weekday::Mon, 9, 0, now);

        // Must land on Monday 09:00 Brisbane == Sunday 23:00 UTC (one hour later), NOT a week on.
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 8, 2, 23, 0, 0).unwrap());
    }

    /// Inside the window the schedule reads active, in local terms (Monday 10:00 Brisbane).
    #[test]
    fn active_inside_a_local_window() {
        let mut s = Schedule::new("bne", false, "Australia/Brisbane", 1).unwrap();
        s.add_weekly_time_range(Weekday::Mon, "09:00", "17:00")
            .unwrap();
        // Monday 2026-08-03 10:00 Brisbane == Monday 00:00 UTC.
        let now = Utc.with_ymd_and_hms(2026, 8, 3, 0, 0, 0).unwrap();
        let start = s.time_on_weekday(Weekday::Mon, 9, 0, now);
        let stop = s.time_on_weekday(Weekday::Mon, 17, 0, now);
        assert!(
            now > start && now < stop,
            "expected inside the Monday window"
        );
    }

    /// A UTC schedule is unaffected by the change (no zone → UTC reference, as before).
    #[test]
    fn utc_schedule_unchanged() {
        let mut s = Schedule::new("utc", true, "", 1).unwrap();
        s.add_weekly_time_range(Weekday::Mon, "09:00", "17:00")
            .unwrap();
        let now = Utc.with_ymd_and_hms(2026, 8, 3, 8, 0, 0).unwrap(); // Mon 08:00 UTC
        let start = s.time_on_weekday(Weekday::Mon, 9, 0, now);
        assert_eq!(start, Utc.with_ymd_and_hms(2026, 8, 3, 9, 0, 0).unwrap());
    }
}
