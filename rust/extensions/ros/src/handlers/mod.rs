//! The MCP verb handlers for the ros sidecar. Grouped one file per **resource** (`ros`, `network`,
//! `device`, `point`) rather than one file per verb: each file stays well under the 400-line limit and
//! keeps a resource's `list/get/create/update/delete` together where they share the same parse +
//! shadow/RosApi plumbing — the "folder-of-verbs" spirit of FILE-LAYOUT without 25 near-empty files.
//! (If any file approaches the limit as later slices add verbs, it splits per-verb then.)
//!
//! Every handler follows the SAME shape, in this order (the scope's contract):
//!   1. **capability self-check** — `host.require("<verb>")` against the sidecar's own grant (see
//!      `host.rs`: the inbound `native.call` carries no caller identity, so the fine-grained gate is
//!      here). A denial refuses BEFORE any REST call or callback.
//!   2. **workspace** is structural (the sidecar's token is ws-scoped; every callback is walled by it).
//!   3. proxy `RosApi` (tree reads/writes) and/or the config shadow (connection records) + secrets.
//!
//! The dispatcher (`dispatch`) is called from `call.rs`; it returns the verb's JSON result string or a
//! typed `HostError` the loop renders as a `Reply::err`.

mod device;
mod network;
mod point;
mod poll;
mod ros;

use std::sync::Arc;

use serde_json::Value;

use crate::host::{HostCtx, HostError};
use crate::poller::run::PollRegistry;
use crate::resolve::RosApiFactory;

/// Parse the opaque-JSON `input` string a `call` carries into a `Value` (empty/absent → `{}`).
pub fn parse_input(input: &str) -> Result<Value, HostError> {
    if input.trim().is_empty() {
        return Ok(Value::Object(Default::default()));
    }
    serde_json::from_str(input).map_err(|e| HostError::BadResponse(format!("input json: {e}")))
}

/// Dispatch a CRUD/ping verb to its resource handler. Returns the verb's JSON result as a string
/// (what the sidecar puts in `Reply::ok`). An unknown verb is `Ok(None)` so `call.rs` can fall through
/// to the poller verbs (slice 3) / point.write (slice 4) before answering "unknown tool".
pub async fn dispatch(
    host: &HostCtx,
    factory: &dyn RosApiFactory,
    registry: &Arc<PollRegistry>,
    tool: &str,
    input: &Value,
    ts: u64,
) -> Result<Option<String>, HostError> {
    let out = match tool {
        "ros.list" => ros::list(host, input).await?,
        "ros.get" => ros::get(host, input).await?,
        "ros.create" => ros::create(host, input, ts).await?,
        "ros.update" => ros::update(host, input, ts).await?,
        "ros.delete" => ros::delete(host, input).await?,
        "ros.ping" => ros::ping(host, factory, input).await?,

        "network.list" => network::list(host, factory, input).await?,
        "network.get" => network::get(host, factory, input).await?,

        "device.list" => device::list(host, factory, input).await?,
        "device.get" => device::get(host, factory, input).await?,

        "point.list" => point::list(host, factory, input).await?,
        "point.get" => point::get(host, factory, input).await?,
        "point.write" => point::write(host, factory, input, ts).await?,

        "ros.start" => poll::start(host, factory, registry, input).await?,
        "ros.stop" => poll::stop(host, registry, input).await?,
        "ros.status" => poll::status(host, registry, input).await?,
        "ros.restart" => poll::restart(host, factory, registry, input).await?,

        _ => return Ok(None),
    };
    Ok(Some(out.to_string()))
}

/// Extract a required string arg or a typed bad-input error.
pub(crate) fn req_str(input: &Value, key: &str) -> Result<String, HostError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| HostError::BadResponse(format!("missing string arg: {key}")))
}

/// Optional keyset cursor + limit from a `list` input.
pub(crate) fn page_args(input: &Value) -> (Option<String>, usize) {
    let cursor = input
        .get("cursor")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let limit = input
        .get("limit")
        .and_then(|v| v.as_u64())
        .map(|n| n as usize)
        .unwrap_or(crate::paging::DEFAULT_LIMIT);
    (cursor, limit)
}
