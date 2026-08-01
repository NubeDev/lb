use crate::schedule::Schedule;
use crate::types::{CombinedStats, ScheduleState};

/// Combines multiple `Schedule` instances with priority-based resolution.
///
/// Exceptions always beat weekly entries within a single schedule. Across
/// schedules, higher `priority` values win. Typical conventions:
/// * `1`  — local / field-device schedule
/// * `10` — master / supervisor schedule
/// * `20` — emergency override
#[derive(Debug, Default)]
pub struct ScheduleEvaluator {
    /// Schedules kept sorted highest-priority first.
    schedules: Vec<Schedule>,
}

impl ScheduleEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a schedule. Re-sorts by priority after insertion.
    pub fn add_schedule(&mut self, s: Schedule) {
        self.schedules.push(s);
        self.sort_by_priority();
    }

    /// Replace the local schedule (priority 1).
    pub fn set_local_schedule(&mut self, mut s: Schedule) {
        s.priority = 1;
        self.remove_by_priority(1);
        self.add_schedule(s);
    }

    /// Replace the master schedule (priority 10).
    pub fn set_master_schedule(&mut self, mut s: Schedule) {
        s.priority = 10;
        self.remove_by_priority(10);
        self.add_schedule(s);
    }

    /// Remove all schedules with the given priority.
    pub fn remove_by_priority(&mut self, priority: i32) {
        self.schedules.retain(|s| s.priority != priority);
    }

    /// Returns `true` if any schedule is currently active.
    pub fn is_active(&self) -> bool {
        self.get_state().is_active
    }

    /// Evaluate all schedules and return the winning state.
    ///
    /// Exceptions are checked before weekly entries. The highest-priority
    /// active entry wins; ties go to the first schedule in sorted order.
    pub fn get_state(&self) -> ScheduleState {
        // Check exceptions first (highest priority first)
        for schedule in &self.schedules {
            if let Some(ex) = schedule.active_exception() {
                return ScheduleState {
                    is_active: true,
                    active_source: format!("{}-{}", schedule.name, ex.source),
                    next_transition: ex.next_stop,
                    current_schedule: Some(schedule.name.clone()),
                    active_priority: ex.priority,
                };
            }
        }

        // No active exception — check weekly entries
        for schedule in &self.schedules {
            for ws in schedule.check_weekly() {
                if ws.is_active {
                    return ScheduleState {
                        is_active: true,
                        active_source: format!("{}-weekly", schedule.name),
                        next_transition: ws.next_stop,
                        current_schedule: Some(schedule.name.clone()),
                        active_priority: ws.priority,
                    };
                }
            }
        }

        ScheduleState {
            is_active: false,
            active_source: "none".to_string(),
            next_transition: self.next_transition(),
            current_schedule: None,
            active_priority: 0,
        }
    }

    /// Returns the earliest upcoming start across all schedules.
    pub fn next_transition(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        let mut earliest: Option<chrono::DateTime<chrono::Utc>> = None;

        for schedule in &self.schedules {
            let candidates = schedule
                .check_exceptions()
                .into_iter()
                .filter_map(|s| s.next_start)
                .chain(
                    schedule
                        .check_weekly()
                        .into_iter()
                        .filter_map(|s| s.next_start),
                );

            for t in candidates {
                match earliest {
                    None => earliest = Some(t),
                    Some(e) if t < e => earliest = Some(t),
                    _ => {}
                }
            }
        }

        earliest
    }

    /// Returns combined stats for every schedule, keyed by name.
    pub fn all_statuses(&self) -> std::collections::HashMap<String, CombinedStats> {
        self.schedules
            .iter()
            .map(|s| (s.name.clone(), s.check_combined()))
            .collect()
    }

    fn sort_by_priority(&mut self) {
        self.schedules
            .sort_by_key(|s| std::cmp::Reverse(s.priority));
    }
}

impl Schedule {
    /// Returns the combined weekly + exception status for this schedule.
    pub fn check_combined(&self) -> CombinedStats {
        let weekly = self.check_weekly();
        let exception = self.check_exceptions();

        let weekly_active = weekly.iter().any(|s| s.is_active);
        let exception_active = exception.iter().any(|s| s.is_active);

        CombinedStats {
            weekly_active,
            exception_active,
            weekly,
            exception,
            priority: self.priority,
        }
    }
}
