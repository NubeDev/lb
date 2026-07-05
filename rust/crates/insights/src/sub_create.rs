//! `sub_create` — create a subscription (insight-subscriptions-scope.md).
//!
//! Requires the caller hold `bus:chan/{channel}:pub` AT CREATE TIME (no-widening up front) in
//! addition to the verb cap. The stored principal is the caller's snapshot, **re-checked at fire
//! time**. Enforces the workspace sub cap (`Policy.sub_cap`, default 1000) — exceeding is a
//! `BadInput` reject.
//!
//! **STUB**: the cap-count + store-write body is deferred — see the punch-list.

use lb_store::Store;

use crate::error::InsightsError;
use crate::subscription::{SubFilter, SubSink};

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
    _store: &Store,
    _ws: &str,
    _owner: &str,
    _principal: &serde_json::Value,
    _input: CreateInput,
    _sub_cap: usize,
    _created_ts: u64,
) -> Result<String, InsightsError> {
    // 1. The host layer has ALREADY checked the caller holds `bus:chan/{channel}:pub` (the
    //    no-widening-up-front gate) + `mcp:insight.sub.create:call`. This verb trusts that.
    // 2. Count existing subs in ws; if ≥ sub_cap ⇒ BadInput ("ws sub cap reached").
    // 3. Mint ULID; write the Subscription row (owner+principal from the caller, NOT the body).
    // 4. Return the id.
    todo!("insights: sub create (cap + write) — SCOPE: subscriptions-scope.md §Verb surface")
}
