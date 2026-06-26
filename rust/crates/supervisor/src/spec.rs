//! The supervision **spec** — what to spawn and how to keep it alive (native-tier scope). A pure
//! value derived from the manifest's `[native]` block (exec/args/env) plus the restart policy; the
//! `Sidecar` reads it and never mutates it. Holds no live state — the running child is the
//! `Sidecar`'s job; this is the recipe, re-read verbatim on every (re)spawn so a restart is
//! identical to the first spawn (the stateless-extension rule applied to a process).

use std::collections::HashMap;
use std::time::Duration;

/// How a child is (re)started after it exits. The crash path; an operator `restart` is a separate
/// cooperative stop→start the host issues, not this policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartPolicy {
    /// Restart only when the child exited abnormally (crash). The default for a sidecar.
    OnCrash,
    /// Never auto-restart — a one-shot child.
    Never,
}

/// Bounded exponential backoff between restarts, with a cap on restarts in a window so a
/// crash-looping child is not respawned forever (native-tier risk: a crash loop must be capped).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Backoff {
    /// The first delay; each subsequent restart doubles it up to `max_delay`.
    pub base: Duration,
    pub max_delay: Duration,
    /// The most restarts allowed before the supervisor gives up (within one `Sidecar`'s life).
    pub max_restarts: u32,
}

impl Backoff {
    /// The delay before restart number `n` (1-based): `base * 2^(n-1)`, capped at `max_delay`.
    pub fn delay_for(&self, restart_number: u32) -> Duration {
        let shift = restart_number.saturating_sub(1).min(16);
        let scaled = self
            .base
            .checked_mul(1u32 << shift)
            .unwrap_or(self.max_delay);
        scaled.min(self.max_delay)
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self {
            base: Duration::from_millis(50),
            max_delay: Duration::from_secs(5),
            max_restarts: 5,
        }
    }
}

/// The recipe for one supervised child: the binary, its args/env, the health-poll interval, the
/// cooperative-shutdown grace window, and the restart policy + backoff. Built from the manifest's
/// `[native]` block by the host; injected identity (workspace/ext/token) is added to `env` at spawn.
#[derive(Debug, Clone)]
pub struct Spec {
    /// Path to the executable to run (the manifest's `[native] exec`, resolved by the host).
    pub exec: String,
    pub args: Vec<String>,
    /// Base environment for the child. The host adds `LB_EXT_WS`/`LB_EXT_ID`/`LB_EXT_TOKEN` on top.
    pub env: HashMap<String, String>,
    /// How often to send a `health` request; a missed reply within this window means restart.
    pub health_interval: Duration,
    /// After a `shutdown` notification, how long to wait before escalating to a process-group kill.
    pub shutdown_grace: Duration,
    pub restart: RestartPolicy,
    pub backoff: Backoff,
}

impl Spec {
    /// A minimal spec for `exec` with default supervision knobs. Args/env/timings are set with the
    /// builder-style setters or by the host from the manifest.
    pub fn new(exec: impl Into<String>) -> Self {
        Self {
            exec: exec.into(),
            args: Vec::new(),
            env: HashMap::new(),
            health_interval: Duration::from_millis(200),
            shutdown_grace: Duration::from_millis(500),
            restart: RestartPolicy::OnCrash,
            backoff: Backoff::default(),
        }
    }

    /// Set the child's argv (after the exec).
    pub fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Add one environment variable the child is spawned with.
    pub fn with_env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.env.insert(key.into(), val.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_then_caps() {
        let b = Backoff {
            base: Duration::from_millis(50),
            max_delay: Duration::from_millis(400),
            max_restarts: 10,
        };
        assert_eq!(b.delay_for(1), Duration::from_millis(50));
        assert_eq!(b.delay_for(2), Duration::from_millis(100));
        assert_eq!(b.delay_for(3), Duration::from_millis(200));
        assert_eq!(b.delay_for(4), Duration::from_millis(400));
        // capped
        assert_eq!(b.delay_for(5), Duration::from_millis(400));
        assert_eq!(b.delay_for(100), Duration::from_millis(400));
    }
}
