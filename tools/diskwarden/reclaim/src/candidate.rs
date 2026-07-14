//! One reclaimable thing a reclaimer found.
//!
//! A `Candidate` is a *finding*, never an action. Producing one is read-only and
//! commits to nothing; `Reclaimer::reclaim` is the only thing that ever deletes.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::size::{now_secs, Measured};

/// A directory (or file) that could be freed, with the facts needed to decide.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    /// Id of the reclaimer that found it (e.g. `"cargo-target"`).
    pub kind: String,
    /// What to delete.
    pub path: PathBuf,
    /// A short human name for the UI (e.g. the project the target belongs to).
    pub label: String,
    /// Bytes that would be freed.
    pub bytes: u64,
    /// Newest mtime in the tree, seconds since the unix epoch.
    pub last_used_secs: u64,
}

impl Candidate {
    pub fn new(
        kind: impl Into<String>,
        path: PathBuf,
        label: impl Into<String>,
        m: Measured,
    ) -> Self {
        Self {
            kind: kind.into(),
            path,
            label: label.into(),
            bytes: m.bytes,
            last_used_secs: m.last_used_secs,
        }
    }

    /// Whole days since anything here was touched, relative to `now`.
    ///
    /// Saturating: a clock skew that puts `last_used_secs` in the future reports 0
    /// (brand new) rather than underflowing into "ancient, delete me".
    pub fn age_days_at(&self, now_secs: u64) -> u64 {
        now_secs.saturating_sub(self.last_used_secs) / 86_400
    }

    /// Whole days since anything here was touched.
    pub fn age_days(&self) -> u64 {
        self.age_days_at(now_secs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(last_used_secs: u64) -> Candidate {
        Candidate {
            kind: "cargo-target".into(),
            path: "/tmp/x/target".into(),
            label: "x".into(),
            bytes: 1,
            last_used_secs,
        }
    }

    #[test]
    fn age_is_whole_days_since_last_touch() {
        let now = 100 * 86_400;
        assert_eq!(candidate(now).age_days_at(now), 0);
        assert_eq!(candidate(now - 86_400).age_days_at(now), 1);
        assert_eq!(candidate(now - 45 * 86_400).age_days_at(now), 45);
    }

    #[test]
    fn a_partial_day_does_not_round_up_into_staleness() {
        let now = 100 * 86_400;
        assert_eq!(candidate(now - 86_399).age_days_at(now), 0);
    }

    /// Clock skew must never manufacture staleness — that direction deletes data.
    #[test]
    fn a_future_mtime_reports_zero_not_underflow() {
        let now = 100 * 86_400;
        assert_eq!(candidate(now + 10 * 86_400).age_days_at(now), 0);
    }
}
