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
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;
    use lb_bus::Bus;
    use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
    use tokio::sync::{Mutex, Notify};

    use crate::registry::Hosted;

    use super::*;

    /// Blocks on its FIRST call until released, then answers instantly on any call after — lets
    /// a test hold an instance's real lock for a controlled window without racing on timing, and
    /// without hanging forever once a nested call reaches it a second time.
    struct Blocking {
        started: Arc<Notify>,
        release: Arc<Notify>,
        blocked_once: Arc<AtomicBool>,
    }

    #[async_trait]
    impl LocalDispatch for Blocking {
        async fn call_tool(
            &mut self,
            _ws: &str,
            _tool: &str,
            _input_json: &str,
            _ctx: Option<CallContext>,
        ) -> Result<String, RuntimeError> {
            if !self.blocked_once.swap(true, Ordering::SeqCst) {
                self.started.notify_one();
                self.release.notified().await;
            }
            Ok("{}".into())
        }
    }

    /// The well-behaved case: answers instantly, never blocks.
    struct Immediate;

    #[async_trait]
    impl LocalDispatch for Immediate {
        async fn call_tool(
            &mut self,
            _ws: &str,
            _tool: &str,
            _input_json: &str,
            _ctx: Option<CallContext>,
        ) -> Result<String, RuntimeError> {
            Ok("{}".into())
        }
    }

    fn hosted(instance: impl LocalDispatch + 'static) -> Hosted {
        Hosted {
            tools: vec![],
            instance: Arc::new(Mutex::new(instance)),
        }
    }

    fn instance_ptr(target: &Target) -> usize {
        match target {
            Target::Local(h) => Arc::as_ptr(&h.instance) as *const () as usize,
            Target::Remote { .. } => unreachable!("test targets are always Local"),
        }
    }

    /// THE bug this guards (`docs/debugging/mcp/gauge-panel-loses-extension-busy-race.md`): a
    /// nested call that targets a DIFFERENT extension than any this call chain already holds
    /// must WAIT for its lock like a top-level call, never fail fast — even under genuine
    /// concurrent contention on that lock. This is the Gauge-vs-Slider shape: `viz.query_batch`
    /// (holding some unrelated instance, stood in by `ptr_a` below) nests a call into `ros`
    /// (`target_b`) while an independent top-level call is already mid-flight against `ros`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nested_call_to_a_different_ext_waits_instead_of_failing_fast() {
        let bus = Bus::peer().await.unwrap();
        let started = Arc::new(Notify::new());
        let release = Arc::new(Notify::new());
        let target_b = Target::Local(hosted(Blocking {
            started: started.clone(),
            release: release.clone(),
            blocked_once: Arc::new(AtomicBool::new(false)),
        }));

        // An unrelated top-level call takes B's lock and holds it…
        let b1 = target_b.clone();
        let bus1 = bus.clone();
        let holder =
            tokio::spawn(async move { dispatch(&b1, &bus1, "ws", "b.tool", "{}", None).await });
        started.notified().await; // …confirmed actually held before proceeding.

        // A nested call (a different ancestor instance, `ptr_a`, already held on THIS task)
        // targets B while it's genuinely contended.
        let b2 = target_b.clone();
        let bus2 = bus.clone();
        let nested = tokio::spawn(async move {
            reentrancy::in_scope(async {
                reentrancy::holding(0xA_usize, dispatch(&b2, &bus2, "ws", "b.tool", "{}", None))
                    .await
            })
            .await
        });

        // Give it ample scheduling time to reach (and register on) B's lock. If dispatch still
        // took the old try_lock/fail-fast branch, it would have resolved (with an error) almost
        // immediately — long before this window elapses.
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(
            !nested.is_finished(),
            "a nested call to a DIFFERENT ext must wait for its lock, not fail fast"
        );

        release.notify_one();
        let nested_result = nested.await.unwrap();
        assert!(
            nested_result.is_ok(),
            "nested call must succeed once the lock frees, got {nested_result:?}"
        );
        holder.await.unwrap().unwrap();
    }

    /// The ORIGINAL protection, unchanged: a call chain re-entering the SAME instance an
    /// ancestor already holds must still fail fast as "extension busy" — awaiting it would
    /// deadlock (nothing else will ever release it).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn nested_call_to_the_same_already_held_ext_fails_fast() {
        let bus = Bus::peer().await.unwrap();
        let target_a = Target::Local(hosted(Immediate));
        let ptr_a = instance_ptr(&target_a);
        let Target::Local(hosted_a) = &target_a else {
            unreachable!()
        };

        // Actually hold the real lock (as an in-flight ancestor call would) AND mark it held on
        // this task — both conditions dispatch() must see to take the fail-fast branch.
        let _guard = hosted_a.instance.lock().await;
        let result = reentrancy::in_scope(async {
            reentrancy::holding(ptr_a, dispatch(&target_a, &bus, "ws", "a.tool", "{}", None)).await
        })
        .await;

        assert!(
            matches!(result, Err(ToolError::Extension(ref m)) if m.contains("extension busy")),
            "self-re-entry into an already-held instance must fail fast, got {result:?}"
        );
    }
}
