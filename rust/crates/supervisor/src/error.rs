//! The supervisor error domain (native-tier scope). Distinct from the host `native` service error:
//! this spans only the OS plumbing — spawn, the framed JSON-RPC line, health, restart.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SupervisorError {
    /// The child could not be spawned (exec missing, not executable, fork failed).
    #[error("failed to spawn child: {0}")]
    Spawn(String),
    /// The framed transport to the child broke — a malformed frame, a closed pipe, or an I/O error.
    /// The supervisor treats this as the child having died (it triggers the restart policy).
    #[error("transport to child failed: {0}")]
    Transport(String),
    /// The child replied with a JSON-RPC error to a request (e.g. an unknown method or a tool error).
    #[error("child returned an error: {0}")]
    Child(String),
    /// The child did not reply within the deadline (handshake or health). Treated as a fault.
    #[error("child timed out: {0}")]
    Timeout(String),
    /// The restart policy was exhausted — the child crash-looped past `max_restarts`. The sidecar is
    /// left stopped; the host surfaces this as a failed lifecycle (no unbounded respawn).
    #[error("restart budget exhausted after {0} restarts")]
    RestartExhausted(u32),
}
