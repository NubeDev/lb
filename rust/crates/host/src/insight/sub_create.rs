//! `insight_sub_create` — create a channel subscription (insight-subscriptions-scope.md).
//!
//! Requires the caller hold `bus:chan/{channel}:pub` AT CREATE TIME (the no-widening-up-front
//! gate) in addition to `mcp:insight.sub.create:call`. `owner` + `principal` are forced to the
//! caller's `sub` + caps snapshot — never caller-supplied.

use lb_auth::Principal;
use lb_caps::Action;
use lb_insights::SubCreateInput;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::InsightSvcError;

/// Create a subscription owned by `principal`. The host checks the channel `pub` grant HERE
/// (create-time no-widening) in addition to the verb cap.
pub async fn insight_sub_create(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: SubCreateInput,
    created_ts: u64,
) -> Result<String, InsightSvcError> {
    authorize_tool(principal, ws, "insight.sub.create").map_err(|_| InsightSvcError::Denied)?;
    // No-widening up front: the caller must hold `bus:chan/{channel}:pub` at create. Uses the same
    // wildcard-aware channel gate `channel::post` runs (so `bus:chan/*:pub` grants any channel),
    // NOT a raw string compare. The fire-time re-check happens in the reactor (reminders pattern).
    crate::channel::authorize_channel(principal, ws, &input.sink.channel, Action::Pub)
        .map_err(|_| InsightSvcError::Denied)?;
    // The principal snapshot stored for fire-time re-check. The caps list is what the host
    // re-runs against `bus:chan/{channel}:pub` at delivery.
    let principal_snapshot = serde_json::to_value(principal.caps()).unwrap_or_default();
    let sub_cap = lb_insights::policy_defaults().sub_cap;
    let id = lb_insights::sub_create(
        store,
        ws,
        principal.sub(),
        &principal_snapshot,
        input,
        sub_cap,
        created_ts,
    )
    .await?;
    Ok(id)
}
