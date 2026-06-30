//! The canonical `coalesce` enum (flow-run-scope "Fan-out posture"). The one vocabulary used
//! **everywhere** a stream is throttled against fan-out: the `event`-trigger node, the dashboard
//! control-inject debounce. Defined here (the run-engine doc owns it); `triggers-lifecycle-scope.md`
//! and `dashboard-binding-scope.md` **reference** this rather than redefining it.
//!
//! A chatty source (an MQTT topic at 1 kHz) must not spawn one run per packet. The coalesce window
//! collapses a burst within `window_ms` into a single firing according to `strategy`.

use serde::{Deserialize, Serialize};

/// How a burst within `window_ms` collapses to a single firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CoalesceStrategy {
    /// Latest-wins within the window: fire with the last value seen when the window closes.
    Latest,
    /// Fire on the first edge of the window, then suppress the rest (leading-edge).
    Leading,
    /// Fire once at the end of the window (trailing-edge).
    Trailing,
    /// Fire at most once per `window_ms` (the first sample), dropping the rest.
    Sample,
}

impl Default for CoalesceStrategy {
    fn default() -> Self {
        Self::Latest
    }
}

/// A coalesce window: a strategy + a duration in ms. `window_ms = 0` means "no coalescing" (one run
/// per event) — opt-in explicitly; the default posture for a chatty source is a non-zero window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Coalesce {
    #[serde(default)]
    pub strategy: CoalesceStrategy,
    #[serde(default)]
    pub window_ms: u64,
}

impl Default for Coalesce {
    fn default() -> Self {
        Self { strategy: CoalesceStrategy::Latest, window_ms: 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults() {
        let c = Coalesce::default();
        assert_eq!(c.strategy, CoalesceStrategy::Latest);
        assert_eq!(c.window_ms, 0);
    }

    #[test]
    fn round_trips() {
        let json = serde_json::json!({"strategy": "trailing", "windowMs": 250});
        let c: Coalesce = serde_json::from_value(json).unwrap();
        assert_eq!(c.strategy, CoalesceStrategy::Trailing);
        assert_eq!(c.window_ms, 250);
    }
}
