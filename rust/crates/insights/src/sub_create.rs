//! `sub_create` — create a subscription (insight-subscriptions-scope.md).
//!
//! Requires the caller hold `bus:chan/{channel}:pub` AT CREATE TIME (no-widening up front) in
//! addition to the verb cap. The stored principal is the caller's snapshot, **re-checked at fire
//! time**. Enforces the workspace sub cap (`Policy.sub_cap`, default 1000) — exceeding is a
//! `BadInput` reject.
//!
//! **STUB**: the cap-count + store-write body is deferred — see the punch-list.

use lb_store::{new_ulid, write, Store};

use crate::error::InsightsError;
use crate::subscription::{SubFilter, SubSink, Subscription, TABLE};
use crate::table_scan::scan_all;

/// The create input (the host fills `owner` + `principal` from the caller's token, never the
/// request body — a caller can't subscribe on another member's behalf).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CreateInput {
    pub sink: SubSink,
    pub filter: SubFilter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub throttle_override: Option<crate::policy::ThrottleOverride>,
}

/// Create a subscription owned by `owner` (the caller's `sub`) with the stored `principal` (their
/// caps snapshot, re-checked at fire). Returns the new sub's id.
// SCOPE: docs/scope/insights/insight-subscriptions-scope.md §"Verb surface" + §"The raise-time matcher"
pub async fn sub_create(
    store: &Store,
    ws: &str,
    owner: &str,
    principal: &serde_json::Value,
    input: CreateInput,
    sub_cap: usize,
    created_ts: u64,
) -> Result<String, InsightsError> {
    // The host layer already checked the caller holds `bus:chan/{channel}:pub` (no-widening up
    // front) + `mcp:insight.sub.create:call`. This verb enforces the workspace sub cap.
    let existing = scan_all(store, ws, TABLE).await?;
    if existing.len() >= sub_cap {
        return Err(InsightsError::BadInput(format!(
            "workspace subscription cap reached ({sub_cap})"
        )));
    }
    let id = new_ulid();
    let sub = Subscription {
        id: id.clone(),
        owner: owner.to_string(),
        principal: principal.clone(),
        sink: input.sink,
        filter: input.filter,
        muted: false,
        throttle_override: input.throttle_override,
        created_ts,
        dormant_reason: None,
    };
    let value = serde_json::to_value(&sub)
        .map_err(|e| InsightsError::Store(lb_store::StoreError::Decode(e.to_string())))?;
    write(store, ws, TABLE, &id, &value).await?;
    Ok(id)
}
