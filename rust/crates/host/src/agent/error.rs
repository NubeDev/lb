//! The agent service's error type. Mirrors the channel/asset error discipline: a denial is opaque
//! (no existence signal), store errors carry through, and a malformed invocation is `BadInput`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentError {
    /// A gate refused (the MCP invoke gate, or a tool/skill/doc the derived principal lacked).
    /// Opaque on purpose — the caller cannot tell "not allowed" from "absent".
    #[error("denied")]
    Denied,
    /// The session (job) was not found in this workspace — e.g. a resume of a missing/cross-ws job.
    #[error("session not found")]
    NotFound,
    /// The invocation arguments were malformed.
    #[error("bad input: {0}")]
    BadInput(String),
    /// A store operation failed underneath.
    #[error("store error: {0}")]
    Store(#[from] lb_store::StoreError),
    /// A persona pinned a grounding skill the run's principal cannot load (ungranted / unreadable).
    /// Fail-closed: the run is refused at start, before any model spend (persona-model scope,
    /// "an ungranted pinned skill fails the run at start with the named error"). Named (not opaque
    /// `Denied`) because the CALLER chose this persona and must see *why* it won't run.
    #[error("persona {persona:?} pins skill {skill:?} which is not granted in this workspace")]
    PersonaSkill { persona: String, skill: String },
    /// A persona restricts the runtimes it may run under (persona-coding #4) and the resolved runtime
    /// is not among them — e.g. the extension-builder paired with an external runtime before the
    /// sandbox ships. Refused at start with the named error.
    #[error("persona {persona:?} may not run on runtime {runtime:?} (allowed: {allowed:?})")]
    PersonaRuntime {
        persona: String,
        runtime: String,
        allowed: Vec<String>,
    },
}
