//! `Severity` — the closed v1 severity set (insights umbrella scope).
//!
//! Closed at `info | warning | critical` until a vertical proves a fourth level (umbrella scope
//! open question #2). Extra dimensions ride the tag graph (e.g. `kind:short-cycle`), never a
//! new variant. The ordering (`info < warning < critical`) drives the matcher's `severity_min`
//! floor (subscriptions scope) and the ladder's severity-escalation breakthrough (notify scope).

use serde::{Deserialize, Serialize};

/// A closed severity set. Ordered low→high for `severity_min` filters + escalation checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Critical,
}

impl Severity {
    /// Total ordering: `Info < Warning < Critical`. Used by the matcher floor + escalation.
    pub fn rank(self) -> u8 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 1,
            Severity::Critical => 2,
        }
    }

    /// True when `self` is at least as severe as `floor` (the subscription `severity_min` check).
    pub fn at_least(self, floor: Severity) -> bool {
        self.rank() >= floor.rank()
    }

    /// The more severe of two — the digest's `max_severity` rollup uses this.
    pub fn max(self, other: Severity) -> Severity {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }
}
