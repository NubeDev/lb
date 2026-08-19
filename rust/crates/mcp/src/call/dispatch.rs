//! The dispatch phase — invoke the tool, locally or across the bus (the S3 routing seam).
//!
//! This is where "edge↔hub becomes real" for tool calls. The target resolved on THIS node is
//! either:
//!   - [`Target::Local`] — lock the in-process instance and call through the WIT `tool.call`;
//!   - [`Target::Remote`] — `query` the hosting node over the workspace-scoped queryable
//!     (`route::call_key`), passing the qualified tool + input, and unwrap its reply.
//!
//! Callers and `authorize` are unchanged from S1 — authorization already ran on this node,
//! workspace-first, before we ever reach here. The remote node re-runs the *local* dispatch
//! when it answers (it never trusts an unauthorized call: the query only reaches its queryable
//! for the workspace in the key, which is the authorized principal's own workspace).

use std::sync::Arc;

use lb_bus::{query, Bus, BusError, NodeId};
use lb_runtime::{CallContext, RuntimeError};

use crate::registry::Target;
use crate::route::{node_call_key, CallReply, CallRequest};

use super::error::ToolError;
use super::reentrancy;

/// Dispatch `qualified_tool`'s call to `target`. Local targets call the instance directly;
/// remote targets route over the bus to the hosting node. `bus` and `ws` are needed only for
/// the remote path (the workspace scopes the routing key).
///
/// `ctx` (the host-callback context) is installed into a **local** instance only — for the duration
/// of this one call, then cleared (`instance.call_tool_with`). A remote target gets none: the guest
/// runs on the other node and its callback identity would have to ride the wire (a separate scope).
pub async fn dispatch(
    target: &Target,
    bus: &Bus,
    ws: &str,
    qualified_tool: &str,
    input_json: &str,
    ctx: Option<CallContext>,
) -> Result<String, ToolError> {
    match target {
        Target::Local(hosted) => {
            // The guest receives the *unqualified* tool name (the `<ext>.` prefix is the host's
            // routing concern, not the extension's).
            let tool = unqualify(qualified_tool);
            // Borrow discipline (host-callback scope, the re-entrancy hazard, corrected —
            // `docs/debugging/mcp/gauge-panel-loses-extension-busy-race.md`): there is ONE
            // instance per ext behind this mutex. Awaiting it only deadlocks when a call chain
            // re-enters the SAME instance an ancestor already holds — a nested call to a
            // DIFFERENT ext cannot deadlock this chain, so it waits like a top-level call
            // (`reentrancy::is_held` answers exactly this, per-task, from the `HELD` scope
            // `reentrancy::in_scope` opens below). Only genuine self-re-entrancy `try_lock`s and
            // fails fast as "extension busy" instead of hanging; the depth guard (`MAX_CALL_DEPTH`
            // in `lb-host`) separately bounds legitimate cross-instance re-entrant chains.
            let ptr = Arc::as_ptr(&hosted.instance) as *const () as usize;
            reentrancy::in_scope(async {
                // ONE short acquisition: ask whether the target multiplexes and, if it does, build
                // its detached future — both under the same guard, which is then dropped. A
                // re-entrant chain must not block here, hence the `try_lock` branch; a failed
                // `try_lock` means self-re-entry, which falls through to the exclusive path below
                // and fails fast as "extension busy" exactly as before.
                let detached = {
                    let guard = if reentrancy::is_held(ptr) {
                        match hosted.instance.try_lock() {
                            Ok(g) => Some(g),
                            Err(_) => None,
                        }
                    } else {
                        Some(hosted.instance.lock().await)
                    };
                    guard.and_then(|g| {
                        g.is_multiplexing()
                            .then(|| g.call_tool_detached(ws, tool, input_json, ctx.clone()))
                            .flatten()
                    })
                };

                // A MULTIPLEXING target (a native sidecar — a separate process whose wire serves
                // many calls at once, correlating replies by `id`) must not be serialised by the
                // per-instance mutex. That mutex exists for a wasm instance, which genuinely cannot
                // run two calls; holding it across a sidecar round-trip caps the whole extension at
                // concurrency 1 node-wide, however well the child multiplexes underneath.
                //
                // Measured on a live node before this branch existed (ext-esr, database confirmed
                // idle): a 20 ms LOCAL store read took 8.96 s behind one slow verb, and three cheap
                // calls issued in parallel released at the SAME instant — a mutex signature, not a
                // queue. See ext-esr/docs/debugging/backend/sidecar-head-of-line-blocking.md.
                //
                // `call_tool_shared` takes `&self`, so it runs with NO lock held. `holding` still
                // wraps it: the re-entrancy guard tracks the call CHAIN, not the mutex, and a
                // sidecar can still call back into the host and re-enter itself.
                // The guard is dropped by now, so the round-trip runs UNLOCKED — that release is
                // the whole fix. A target that claims to multiplex but offers no detached path
                // yields `None` here and simply falls through to the exclusive path below: always
                // correct, only slower.
                if let Some(fut) = detached {
                    return reentrancy::holding(ptr, fut).await.map_err(map_err);
                }

                let mut instance = if reentrancy::is_held(ptr) {
                    hosted.instance.try_lock().map_err(|_| {
                        ToolError::Extension("extension busy (re-entrant call)".into())
                    })?
                } else {
                    hosted.instance.lock().await
                };
                reentrancy::holding(ptr, async {
                    instance
                        .call_tool(ws, tool, input_json, ctx)
                        .await
                        .map_err(map_err)
                })
                .await
            })
            .await
        }
        Target::Remote { node, .. } => route(bus, ws, node, qualified_tool, input_json).await,
    }
}

/// Route a call to the node hosting `qualified_tool`'s extension over the bus queryable.
///
/// **Always dispatches on the NODE-QUALIFIED key** (`mcp/{ext}/{node}/call`), never the shared
/// `mcp/{ext}/call` — resolve always knows the node, even for an untargeted call to a singly-hosted
/// ext, so there is no case left that needs the fan-in key. This closes the residual coin flip for
/// a caller whose registry knows only one host while a second is live: on the shared key such a
/// caller would resolve "unambiguously" and still race two responders (scope, Risks — "the shared
/// key cannot simply be deleted, but it can stop carrying calls").
async fn route(
    bus: &Bus,
    ws: &str,
    node: &NodeId,
    qualified_tool: &str,
    input_json: &str,
) -> Result<String, ToolError> {
    let ext = qualified_tool
        .split_once('.')
        .map(|(e, _)| e)
        .unwrap_or(qualified_tool);
    let req = CallRequest {
        tool: qualified_tool.to_string(),
        input: input_json.to_string(),
    };
    let bytes = serde_json::to_vec(&req).map_err(|e| ToolError::BadInput(e.to_string()))?;

    let reply = query(bus, ws, &node_call_key(ext, node), &bytes)
        .await
        .map_err(|e| match e {
            // Two nodes answered a key only one node should declare. `lb_bus::query` catches this
            // at the call site (see its `MultipleResponders`); surfacing it as a distinct error
            // rather than silently keeping the first reply is the runtime half of the "exactly one
            // responder" invariant. It means two nodes are announcing the SAME node id — a
            // provisioning fault that must be loud.
            BusError::MultipleResponders { .. } => ToolError::Extension(format!(
                "routing fault: more than one node answered for {node} — duplicate node id?"
            )),
            other => ToolError::Extension(format!("route: {other}")),
        })?
        // Zero responders on a node-qualified key: that node is not here. This is the primary
        // unreachability signal (scope, open question 8) — a `get` against a key with no matching
        // queryable completes fast, so this is a prompt refusal, not a timeout. It is a REFUSAL:
        // never a queue, never a fallback to another host of the same ext.
        .ok_or_else(|| ToolError::NodeUnreachable {
            node: node.to_string(),
        })?;

    match serde_json::from_slice::<CallReply>(&reply)
        .map_err(|e| ToolError::Extension(format!("bad routed reply: {e}")))?
    {
        CallReply::Ok(output) => Ok(output),
        CallReply::Err(msg) => Err(ToolError::Extension(msg)),
    }
}

/// Strip the `<ext>.` prefix to the unqualified tool name the guest expects.
fn unqualify(qualified_tool: &str) -> &str {
    qualified_tool
        .split_once('.')
        .map(|(_, t)| t)
        .unwrap_or(qualified_tool)
}

fn map_err(e: RuntimeError) -> ToolError {
    match e {
        RuntimeError::Tool(m) => ToolError::Extension(m),
        other => ToolError::Extension(other.to_string()),
    }
}

#[cfg(test)]
#[path = "dispatch_tests.rs"]
mod tests;
