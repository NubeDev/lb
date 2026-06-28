//! Persist + read the [`AgentDecision`] first-settle record (agent-run scope Part 2).
//!
//! Two writes with **deliberately different** semantics:
//!   - `create_pending` uses `lb_store::create` (the conditional **first-write**): opening a
//!     suspension reserves the `{job,tool_call}` key once. A second open on the same key is a
//!     `Conflict` — never a silent re-open (so a re-scan of the loop does not duplicate the pause).
//!   - `flip_to_settled` is the guarded settle: it reads the current record and only writes the
//!     `Settled` state **if it is still `Pending`**. The first `agent.decide` binds; a second finds
//!     it already `Settled` and is rejected; a decide after the tool ran is a no-op. This is the
//!     first-settle invariant — the one a plain `lb_inbox::Resolution` (last-writer-wins) would fail.
//!
//! On the read-modify-write race: the embedded store serializes a single node's writes, and the
//! reactor/decide path is single-actor per node, so the read-then-conditional-write is correct here.
//! If a future multi-writer settle path appears, this is the one place to harden into a single
//! `UPDATE … WHERE state = 'pending'` round-trip (noted, not pre-built — the surface today is one
//! settler).
//!
//! Raw store verbs — *no authorization*. The `agent.decide` MCP verb (gated by `mcp:agent.decide:call`)
//! and the loop call these after their own checks.

use serde_json::Value;

use lb_jobs::SuspensionDecision;
use lb_store::{create, read, write, Store, StoreError};

use super::model::{decision_id, AgentDecision, DecisionState, DECISION_TABLE};

/// The result of attempting to settle a decision — distinguishes the binding settle from the
/// idempotent no-op (an already-settled record), so the caller can branch (and the test can assert
/// "first binds, second rejected").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleOutcome {
    /// This call bound the decision (it was `Pending`); the loop should resume against it.
    Bound(SuspensionDecision),
    /// The decision was already settled — a duplicate `agent.decide` or a reactor re-scan. The prior
    /// outcome is returned; the caller treats this as a no-op (do not double-apply / re-resume).
    AlreadySettled(SuspensionDecision),
}

/// `create` the pending decision (first-write). `Conflict` if the key already exists — an open that
/// races a re-scan reserves the suspension exactly once.
pub async fn create_pending(
    store: &Store,
    ws: &str,
    decision: &AgentDecision,
) -> Result<(), StoreError> {
    let id = decision_id(&decision.job_id, &decision.tool_call_id);
    let value: Value = serde_json::to_value(decision)
        .map_err(|e| StoreError::Backend(format!("encode agent_decision: {e}")))?;
    create(store, ws, DECISION_TABLE, &id, &value).await
}

/// Read a decision record (or `None` if absent / cross-workspace — isolation).
pub async fn load_decision(
    store: &Store,
    ws: &str,
    job_id: &str,
    tool_call_id: &str,
) -> Result<Option<AgentDecision>, StoreError> {
    let id = decision_id(job_id, tool_call_id);
    match read(store, ws, DECISION_TABLE, &id).await? {
        Some(value) => serde_json::from_value(value)
            .map(Some)
            .map_err(|e| StoreError::Backend(format!("decode agent_decision: {e}"))),
        None => Ok(None),
    }
}

/// Conditionally flip a pending decision to `Settled` with `decision`. Returns:
///   - `Bound` if it was `Pending` (this call wins — first-settle binds),
///   - `AlreadySettled` if it was already settled (rejected as a no-op — second decide / re-scan),
///   - `Conflict` if the record does not exist (a settle for an unopened suspension).
pub async fn flip_to_settled(
    store: &Store,
    ws: &str,
    job_id: &str,
    tool_call_id: &str,
    decision: SuspensionDecision,
    ts: u64,
) -> Result<SettleOutcome, StoreError> {
    let mut rec = match load_decision(store, ws, job_id, tool_call_id).await? {
        Some(rec) => rec,
        None => return Err(StoreError::Conflict),
    };
    if rec.state == DecisionState::Settled {
        // Already bound — the duplicate decide / re-scan is a no-op. Return the prior outcome so the
        // caller can stay idempotent without a second write.
        let prior = rec.decision.unwrap_or(decision);
        return Ok(SettleOutcome::AlreadySettled(prior));
    }
    rec.state = DecisionState::Settled;
    rec.decision = Some(decision);
    rec.ts = ts;
    let id = decision_id(job_id, tool_call_id);
    let value: Value = serde_json::to_value(&rec)
        .map_err(|e| StoreError::Backend(format!("encode agent_decision: {e}")))?;
    write(store, ws, DECISION_TABLE, &id, &value).await?;
    Ok(SettleOutcome::Bound(decision))
}
