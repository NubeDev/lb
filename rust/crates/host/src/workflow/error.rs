//! The workflow service error. Same discipline as the channel/asset/agent errors: a denial is
//! opaque (no existence signal), store errors carry through. Adds the one new outcome the S6 gate
//! needs — [`AwaitingApproval`](WorkflowError::AwaitingApproval): the job-start verb refused because
//! the approval is unresolved or not `approved`. That refusal *is* the gate (coding-workflow scope).

use thiserror::Error;

use crate::agent::AgentError;

#[derive(Debug, Error)]
pub enum WorkflowError {
    /// A workflow capability gate refused. Opaque — the caller cannot tell "not allowed" from
    /// "absent" (capability-first, §3.5).
    #[error("denied")]
    Denied,
    /// The coding job cannot start: the approval inbox item is unresolved or not `approved`. This
    /// is the genuine gate — no job record exists until an approval lands (coding-workflow scope).
    #[error("awaiting approval")]
    AwaitingApproval,
    /// A referenced item/doc/job was not found in this workspace (a denied caller gets `Denied`,
    /// never `NotFound`, so this leaks nothing).
    #[error("not found")]
    NotFound,
    /// The agent (triage / the coding loop) failed underneath.
    #[error("agent error: {0}")]
    Agent(#[from] AgentError),
    /// A durable store operation failed.
    #[error("store error: {0}")]
    Store(#[from] lb_store::StoreError),
}
