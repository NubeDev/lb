//! Persist + read the per-workspace [`Policy`] record (agent-run scope Part 2).
//!
//! One record per workspace, addressed `agent_policy:{ws}` (the id is the workspace id — deterministic,
//! so the loop reads it without a lookup and `agent.policy.set` overwrites the same row). Reads default
//! to an empty policy when absent (default-allow — the policy only *adds* gating). The write is an
//! **upsert** (`lb_store::write`): a policy edit replaces the whole rule list, last-writer-wins — this
//! is editing config, not the first-settle decision record (those have opposite requirements, which is
//! exactly why the agent_decision record is separate; see `decision/`).
//!
//! These are the raw store verbs — *no authorization here*. The `agent.policy.set` MCP verb (gated by
//! an admin cap) and the loop call them after their own checks (capability-first, §3.5).

use serde_json::Value;

use lb_store::{read, write, Store, StoreError};

use super::model::{Policy, POLICY_TABLE};

/// Read workspace `ws`'s policy, or an empty (default-allow) policy if none is set. A malformed stored
/// record surfaces as a backend error rather than silently allowing everything (fail closed on a
/// corrupt policy, not open).
pub async fn load_policy(store: &Store, ws: &str) -> Result<Policy, StoreError> {
    match read(store, ws, POLICY_TABLE, ws).await? {
        Some(value) => serde_json::from_value(value)
            .map_err(|e| StoreError::Backend(format!("decode agent_policy: {e}"))),
        None => Ok(Policy::default()),
    }
}

/// Upsert workspace `ws`'s policy (replace the whole rule list). Last-writer-wins is correct for
/// config; the first-settle guarantee lives on the per-call `agent_decision` record, not here.
pub async fn save_policy(store: &Store, ws: &str, policy: &Policy) -> Result<(), StoreError> {
    let value: Value = serde_json::to_value(policy)
        .map_err(|e| StoreError::Backend(format!("encode agent_policy: {e}")))?;
    write(store, ws, POLICY_TABLE, ws, &value).await
}
