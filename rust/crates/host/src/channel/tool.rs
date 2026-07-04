//! `channel.*` MCP verbs — the thin dispatch arms that give channels an MCP contract
//! (rules-messaging-scope). `channel.post`/`history`/`edit`/`delete`/`list` are wrappers over the
//! existing host fns ([`post`], [`history`], [`edit`], [`delete`], [`channel_list`]), each **gate-
//! identical** to the WS/`POST /channels` path: the host fn runs its own `channel/authorize.rs` gate
//! (`bus:chan/{cid}:{Pub|Sub}`, workspace-first) exactly as before — this file adds NO new gate, only a
//! JSON front door so a rule/agent/UI reaches channels through the ONE MCP contract (rule 7) instead of
//! the channel host fns being special-cased in a seam (a rule-10 leak).
//!
//! `channel.post` keeps FULL PARITY with the WS path: a `kind:"query"`/`kind:"agent"` item triggers the
//! inline query / background agent worker exactly as a browser post does (no special-casing — rule 7).
//! The *rule handle* is the stricter layer that fences worker kinds (slice 3); the generic verb stays
//! uniform. **Author is forced to the caller's `sub`** — never request-supplied — so a caller cannot
//! forge another member's authorship (the same contract `inbox.record` holds in `tool_call.rs`).

use lb_auth::Principal;
use lb_inbox::Item;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::error::ChannelError;
use crate::boot::Node;

/// Dispatch a `channel.<verb>` MCP call. The outer `tool_call.rs` gate already ran
/// `mcp:channel.<verb>:call`; each host fn re-runs the channel `Pub`/`Sub` gate inside (defense in
/// depth, and it is the same gate the WS path passes). A deny collapses to the opaque
/// [`ToolError::Denied`]; a genuine `NotFound` (owner, absent id) surfaces as `NotFound`.
pub async fn call_channel_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "channel.post" => {
            let cid = arg_str(input, "cid")?;
            // Author is FORCED to the caller — a request `author` is ignored (never spoofable).
            let item = item_from_input(principal, input)?;
            let stored = super::post(node, principal, ws, cid, item)
                .await
                .map_err(chan_err)?;
            serde_json::to_value(stored).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "channel.history" => {
            let cid = arg_str(input, "cid")?;
            let items = super::history(&node.store, principal, ws, cid)
                .await
                .map_err(chan_err)?;
            // Optional bounded tail — a rule reads a snapshot (`channel.history(cid, n)`), not a watch.
            let items = match input.get("n").and_then(Value::as_u64) {
                Some(n) if (n as usize) < items.len() => items[items.len() - n as usize..].to_vec(),
                _ => items,
            };
            Ok(json!({ "messages": items }))
        }
        "channel.edit" => {
            let cid = arg_str(input, "cid")?;
            let id = arg_str(input, "id")?;
            let body = input.get("body").and_then(Value::as_str).unwrap_or("");
            let ts = input.get("ts").and_then(Value::as_u64).unwrap_or(0);
            let updated = super::edit(node, principal, ws, cid, id, body, ts)
                .await
                .map_err(chan_err)?;
            serde_json::to_value(updated).map_err(|e| ToolError::Extension(e.to_string()))
        }
        "channel.delete" => {
            let cid = arg_str(input, "cid")?;
            let id = arg_str(input, "id")?;
            super::delete(node, principal, ws, cid, id)
                .await
                .map_err(chan_err)?;
            Ok(json!({ "ok": true }))
        }
        "channel.list" => {
            let channels = crate::channel_list(&node.store, principal, ws)
                .await
                .map_err(chan_err)?;
            serde_json::to_value(json!({ "channels": channels }))
                .map_err(|e| ToolError::Extension(e.to_string()))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Build the posted [`Item`] from the input, FORCING the author to the caller's `sub`. `id` is
/// caller-supplied for idempotency; `ts` is the caller's logical clock (no wall-clock in core).
fn item_from_input(principal: &Principal, input: &Value) -> Result<Item, ToolError> {
    let id = arg_str(input, "id")?.to_string();
    let body = input
        .get("body")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let ts = input.get("ts").and_then(Value::as_u64).unwrap_or(0);
    // The kind-tagged payload (query/agent/text) rides INSIDE `body` as JSON exactly as the WS path
    // sends it (payload.rs) — so `post`'s inline query / background agent workers key off it with full
    // parity, no special-casing here. `channel` is filled in by `post` from `cid`.
    let item = Item::new(id, "", principal.sub().to_string(), body, ts);
    Ok(item)
}

/// Read a required string arg or a clean `BadInput`.
fn arg_str<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

/// Map a [`ChannelError`] to the MCP error. `Denied` stays OPAQUE (indistinguishable from a missing
/// tool); `NotFound` surfaces; store/bus faults are extension errors.
fn chan_err(e: ChannelError) -> ToolError {
    match e {
        ChannelError::Denied => ToolError::Denied,
        ChannelError::NotFound => ToolError::NotFound,
        other => ToolError::Extension(other.to_string()),
    }
}
