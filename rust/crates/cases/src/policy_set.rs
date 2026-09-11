//! `policy_set` — upsert one [`ServicePolicy`] row (case-plane scope, `policy.sla.set`).
//!
//! Admin-gated at the host layer (`mcp:policy.sla.set:call`); this layer is the write + the
//! validation. Keyed by `id`, so a second write with the same id replaces the row — the settings
//! surface edits in place and packs re-seed idempotently.
//!
//! The workspace default is simply a row whose `match` is empty; there is no separate "default"
//! table or flag, because a default that is a different kind of thing from a policy is a second
//! resolution path to keep in sync.

use lb_store::{write, Store};

use crate::error::CasesError;
use crate::policy::{ServicePolicy, POLICY_TABLE};

/// Write `policy` into the workspace's `service_policy` table, replacing any row with the same id.
///
/// Validates before it writes: an empty id, a zero-length name, a resolve deadline earlier than the
/// respond deadline, or a calendar the clock could not use is a `BadInput` reject rather than a row
/// that quietly governs work. Validating here rather than at read time is what lets
/// [`crate::deadline`] stay total.
pub async fn policy_set(store: &Store, ws: &str, policy: &ServicePolicy) -> Result<(), CasesError> {
    validate(policy)?;
    let value = serde_json::to_value(policy).map_err(CasesError::decode)?;
    write(store, ws, POLICY_TABLE, &policy.id, &value).await?;
    Ok(())
}

/// The write-door checks. Separated so a caller that wants to pre-flight a form can reuse them.
pub fn validate(policy: &ServicePolicy) -> Result<(), CasesError> {
    if policy.id.trim().is_empty() {
        return Err(CasesError::BadInput("policy id must not be empty".into()));
    }
    // `resolve_h < respond_h` is not a subtle preference — it means the case is due to be FIXED
    // before anyone is due to have LOOKED at it, which makes the queue's ordering nonsense.
    if policy.resolve_h < policy.respond_h {
        return Err(CasesError::BadInput(format!(
            "resolve_h ({}) must not be earlier than respond_h ({})",
            policy.resolve_h, policy.respond_h
        )));
    }
    policy
        .calendar
        .validate()
        .map_err(|e| CasesError::BadInput(format!("calendar: {e}")))?;
    Ok(())
}
