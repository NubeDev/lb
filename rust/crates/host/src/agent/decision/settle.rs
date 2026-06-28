//! `settle_decision` — the `agent.decide {job_id, tool_call_id, decision}` verb's core (agent-run
//! scope Part 2). It binds a pending suspension with **first-settle** semantics and leaves the job
//! **resumable** so the run continues exactly once.
//!
//! The flow:
//!   1. flip the `agent_decision` record `Pending → Settled` *conditionally* (`flip_to_settled`):
//!      the first decide binds (`Bound`); a second finds it settled (`AlreadySettled`, a no-op); a
//!      decide for an unopened call is `Conflict`. **This is the first-settle invariant** — the one a
//!      last-writer-wins `lb_inbox::Resolution` would silently violate.
//!   2. on a fresh bind, `unsuspend` the job (`Suspended → Running`) so the next `resume()` (a direct
//!      call, or the S6-style reactor scan) rehydrates and continues from the cursor. On an
//!      `AlreadySettled` we do NOT touch the job — a duplicate decide / re-scan must not re-arm a run
//!      that may already have resumed (idempotent, no double-apply, no re-spend).
//!
//! Authorization is the caller's (`agent.decide` runs `mcp:agent.decide:call` workspace-first before
//! reaching here) — exactly the authority that resolves the surfaced inbox item, but the binding write
//! is this record, not the inbox row.
//!
//! Why settle does not itself drive the loop: the loop needs a `ModelAccess`, which this verb (reached
//! from the MCP dispatch with no model in hand) does not have. So settle leaves the job `Running` and
//! the resume happens through the existing `resume()` entry (the reactor or the lifecycle client calls
//! it) — the same start/resume seam Part 3 watches. The run applies the now-settled decision on its
//! next pass (see `run.rs`'s resume-mode handling).

use lb_jobs::{unsuspend, SuspensionDecision};
use lb_store::{Store, StoreError};

use super::store::{flip_to_settled, SettleOutcome};

/// Settle the suspension on `{job_id, tool_call_id}` in `ws` with `decision`. Returns the
/// [`SettleOutcome`] so the caller can tell a binding settle from an idempotent duplicate. `ts` is the
/// caller-injected logical settle time (no wall-clock).
pub async fn settle_decision(
    store: &Store,
    ws: &str,
    job_id: &str,
    tool_call_id: &str,
    decision: SuspensionDecision,
    ts: u64,
) -> Result<SettleOutcome, StoreError> {
    let outcome = flip_to_settled(store, ws, job_id, tool_call_id, decision, ts).await?;
    // Only a fresh bind re-arms the job. A duplicate decide leaves the (possibly already-resumed) job
    // untouched — re-arming it would risk a second resume / re-spend.
    if let SettleOutcome::Bound(_) = outcome {
        unsuspend(store, ws, job_id).await?;
    }
    Ok(outcome)
}
