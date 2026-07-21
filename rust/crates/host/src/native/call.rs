//! Dispatch one tool call to a live sidecar, with the shared **crash-restart-on-fault** discipline
//! (native-tier scope). Both the typed lifecycle path (`call_sidecar`) and the Tier-agnostic
//! registry adapter ([`SidecarDispatch`]) go through [`call_once_or_restart`] so the fault handling
//! lives in ONE place, not two.
//!
//! The fault path is: attempt the call; if the child DIED mid-call (`Transport`/`Child`), run the
//! caller-supplied `on_fault` recovery (re-spawn + bump the durable restart count) and retry once.
//! `call_sidecar` supplies a recovery that restarts via the `Launcher` and bumps the store record —
//! the supervision proof. The registry adapter supplies a NO-OP recovery: it is stored node-global
//! with no `Launcher` in hand (the `Launcher` trait is `impl Future`, not object-safe, so the
//! adapter cannot own one), so a transport fault surfaces cleanly to the routed caller rather than
//! silently swallowing supervision. A subsequent lifecycle `restart`/`call` (which DO carry the
//! launcher) then recovers the child — no supervision is lost, it is just driven by the typed path.

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use lb_supervisor::{Caller, Sidecar, SupervisorError};
use tokio::sync::Mutex as AsyncMutex;

use super::registry::SidecarMap;

/// The host-side bound on ONE native tool call (native-call-concurrency scope, promoted from an open
/// question). The federation child bounds its own queries at 30 s (`federation/src/query.rs`), but
/// the host waited **indefinitely** for any native reply, and not every native extension is
/// federation.
///
/// This matters MORE after multiplexing, not less. Serially, one stuck call blocked everybody and was
/// therefore obvious within seconds. Multiplexed, it silently pins one waiter and one in-flight slot
/// while everything else keeps working — capacity degrades invisibly until the child is saturated by
/// calls that will never return.
///
/// 45 s, chosen to sit **above** the child's own 30 s query bound rather than under it: the child's
/// typed "query exceeded the 30s bound" error is a far better answer than an opaque host timeout, so
/// the host bound is a backstop for a child that has stopped answering at all, not a competing
/// deadline that would pre-empt the child's own message. Not manifest-driven yet (see the scope's
/// open questions) — one constant beats a knob nobody sets.
const CALL_TIMEOUT: Duration = Duration::from_secs(45);

/// The bound actually applied. Tests override it (via `#[cfg(test)]`) so the fault/retry paths can
/// be exercised in milliseconds instead of waiting out the production 45 s.
#[cfg(not(test))]
fn call_timeout() -> Duration {
    CALL_TIMEOUT
}
#[cfg(test)]
fn call_timeout() -> Duration {
    Duration::from_millis(200)
}

/// Call `tool` on `handle`'s sidecar; on a mid-call fault, run `on_fault` (recovery) and retry once.
/// The recovery is what differs between the typed path (restart + store bump) and the adapter
/// (no-op) — the attempt/fault/retry shape is shared. `on_fault` runs with the handle unlocked so it
/// may re-lock to restart.
pub(super) async fn call_once_or_restart<F, Fut>(
    handle: &Arc<AsyncMutex<Sidecar>>,
    tool: &str,
    input: &str,
    caller: Option<Caller>,
    on_fault: F,
) -> Result<String, SupervisorError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(), SupervisorError>>,
{
    // Remember WHICH generation the first attempt ran on, so the retry can tell "the child was
    // replaced under me" (worth retrying) from "the same dead child is still installed" (not).
    let (first, first_gen) = attempt(handle, tool, input, caller.clone()).await;
    match first {
        Ok(out) => Ok(out),
        // The child died mid-call (no reply came back): run the recovery, then retry once.
        // `Child(_)` is NOT a fault — it is the child's ordinary error REPLY over a healthy line
        // (a failed SQL query, a bad arg). Restarting on it burned the whole restart budget on
        // five failed queries and took federation dark mid-run (live); the child was never down.
        //
        // `Timeout(_)` IS a fault, and it is new here: before the host-side `CALL_TIMEOUT` existed,
        // a child that stopped answering surfaced as `Transport` (EOF) or hung forever. Now it can
        // surface as a host-bound timeout instead — the same condition ("the child is not
        // answering"), so it must take the same recovery path. Leaving it in the catch-all `other`
        // arm silently removed restart-and-retry for hung children, which is exactly the case
        // supervision exists for. Caught by `a_real_restart_is_still_retried`.
        Err(SupervisorError::Transport(_)) | Err(SupervisorError::Timeout(_)) => {
            on_fault().await?;

            // **Generation-aware retry** (native-call-concurrency scope, Risk 7). Serially this
            // cost one wasted retry; multiplexed it costs N, because a single child death wakes N
            // waiters with `Transport` and every one of them retries. `SidecarDispatch` supplies a
            // NO-OP `on_fault` by design (it holds no `Launcher`), so on that path nothing has
            // recovered the child and all N would re-attempt against the same corpse.
            //
            // So retry only if the installed generation actually CHANGED — i.e. some other path
            // (the typed lifecycle `restart`, the supervision reactor) really did replace the
            // child. Otherwise fail fast with the original error.
            let current_gen = handle.lock().await.generation();
            if current_gen == first_gen {
                return Err(SupervisorError::Transport(
                    "child is down and was not recovered (same generation)".into(),
                ));
            }
            // Re-stamp the same caller on the retry — the restarted child is a fresh process that
            // never saw the first frame, so it must learn the caller again (identity is per-call).
            attempt(handle, tool, input, caller).await.0
        }
        Err(other) => Err(other),
    }
}

/// One bounded attempt. Returns the result **and the generation it ran on**, so the caller can make
/// the retry generation-aware.
///
/// Takes the handle lock only long enough to clone the live `Conn` out, then runs the round-trip
/// **unlocked** under [`CALL_TIMEOUT`].
async fn attempt(
    handle: &Arc<AsyncMutex<Sidecar>>,
    tool: &str,
    input: &str,
    caller: Option<Caller>,
) -> (Result<String, SupervisorError>, u64) {
    // Short lock: clone the live generation's handle out, then RELEASE. `tokio`'s mutex is exclusive
    // regardless of `&self`/`&mut self`, so keeping the guard for the round-trip would re-impose
    // concurrency 1 however well `Conn` multiplexes underneath. The `drop` is the fix — inlining
    // this into `handle.lock().await.call_with_caller(..).await` silently restores the old bug.
    let (conn, generation) = {
        let guard = handle.lock().await;
        (guard.conn(), guard.generation())
    };
    let conn = match conn {
        Ok(c) => c,
        Err(e) => return (Err(e), generation),
    };

    // Unlocked round-trip, bounded by the host deadline.
    let out = match tokio::time::timeout(call_timeout(), conn.call_with_caller(tool, input, caller))
        .await
    {
        Ok(result) => result,
        Err(_) => Err(SupervisorError::Timeout(format!(
            "native call `{tool}` exceeded the {:?} host bound",
            call_timeout()
        ))),
    };
    (out, generation)
}

/// The Tier-agnostic **native-sidecar adapter**: a [`LocalDispatch`] that forwards a routed/local
/// tool call to the live sidecar for `(ws, ext_id)`. Registered into `lb_mcp::Registry` at install so
/// `resolve`/`dispatch`/`serve_call` reach a native sidecar through the ONE trait — no `if native`
/// branch (§3.1).
///
/// It holds `Arc<SidecarMap>` + `ext_id` (NOT a single ws): the registry is node-global, so the ws
/// is resolved **per call** from the `ws` the trait passes — `SidecarMap.get(ws, ext_id)`. A ws-B
/// routed call thus resolves ws-B's child or `None`; a ws-B call can never reach a ws-A sidecar (the
/// workspace wall stays structural for Tier 2, mirroring the map key).
pub struct SidecarDispatch {
    sidecars: Arc<SidecarMap>,
    ext_id: String,
}

impl SidecarDispatch {
    pub(super) fn new(sidecars: Arc<SidecarMap>, ext_id: impl Into<String>) -> Self {
        Self {
            sidecars,
            ext_id: ext_id.into(),
        }
    }
}

#[async_trait::async_trait]
impl LocalDispatch for SidecarDispatch {
    async fn call_tool(
        &mut self,
        ws: &str,
        tool: &str,
        input_json: &str,
        ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        // Resolve the live child for THIS workspace — `None` means not running here (or a ws-mismatch
        // that structurally cannot cross the wall).
        let handle = self
            .sidecars
            .get(ws, &self.ext_id)
            .ok_or_else(|| RuntimeError::Call(format!("no running sidecar for {}", self.ext_id)))?;

        // The registry passed the BARE tool name (`dispatch`/`serve_call` unqualify). Re-qualify it
        // as `<ext_id>.<tool>` for the sidecar's control-line ABI, which dispatches on the qualified
        // name (its manifest declares tools qualified). This mirrors the direct `call_sidecar` path,
        // which passes the qualified name through. Generic: `ext_id` + `.` + the bare tool.
        let qualified = format!("{}.{}", self.ext_id, tool);

        // Stamp the authorized caller into the frame so the child can enforce per-caller row
        // visibility (native-caller-identity scope). Projected from `ctx` — a READ of the principal
        // the host already gated (`mcp:<tool>:call` fired first, workspace-first); NOT a token. A
        // routed cross-node call carries no `ctx` (the serve side passes `None`), so the frame is the
        // old shape — the caller-in-frame guarantee is single-node this slice, matching the scope's
        // non-goal. `LB_EXT_TOKEN` remains the child's identity for its OWN callbacks; the frame
        // caller is a separate, inert projection the child reads to attribute a decision.
        let caller = ctx.and_then(|c| c.caller).map(project);

        // No-op recovery: the adapter carries no `Launcher` (see module doc), so a transport fault
        // surfaces; the typed lifecycle path drives supervised restart.
        call_once_or_restart(&handle, &qualified, input_json, caller, || async { Ok(()) })
            .await
            .map_err(|e| RuntimeError::Tool(e.to_string()))
    }
}

/// Map the runtime's [`lb_runtime::Caller`] (carried on `CallContext`, shared with the wasm path)
/// onto the supervisor's wire [`Caller`] (serialized into the sidecar frame). Two identical minimal
/// projections, one per crate boundary: `lb-runtime` stays free of `lb-supervisor` and vice versa —
/// the host is the one place that bridges the wasm-runtime and native-supervisor worlds.
fn project(c: lb_runtime::Caller) -> Caller {
    Caller {
        sub: c.sub,
        ws: c.ws,
        role: c.role,
        delegated: c.delegated,
        admin: c.admin,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lb_supervisor::{Channel, Kill, Launcher, Method, Reply, Request, Spec};
    use std::collections::HashMap;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct NoKill;
    impl Kill for NoKill {
        fn kill(self: Box<Self>) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
    }

    /// A child that answers `init`, counts `call` frames, then never replies to them.
    struct CountingLauncher {
        calls: Arc<AtomicU32>,
    }

    impl Launcher for CountingLauncher {
        async fn launch(
            &self,
            _exec: &str,
            _args: &[String],
            _env: &HashMap<String, String>,
        ) -> Result<Channel, SupervisorError> {
            let calls = Arc::clone(&self.calls);
            let (host_side, child_side) = tokio::io::duplex(64 * 1024);
            let (mut cr, mut cw) = tokio::io::split(child_side);
            tokio::spawn(async move {
                if let Ok(body) = lb_supervisor::read_frame(&mut cr).await {
                    let req: Request = serde_json::from_slice(&body).unwrap();
                    let bytes = serde_json::to_vec(&Reply::ok(req.id, "ready")).unwrap();
                    let _ = lb_supervisor::write_frame(&mut cw, &bytes).await;
                }
                drop(cw); // every subsequent call fails transport, promptly
                while let Ok(body) = lb_supervisor::read_frame(&mut cr).await {
                    if let Ok(r) = serde_json::from_slice::<Request>(&body) {
                        if r.method == Method::Call {
                            calls.fetch_add(1, Ordering::SeqCst);
                        }
                    }
                }
            });
            let (read, write) = tokio::io::split(host_side);
            Ok(Channel {
                write: Box::pin(write),
                read: Box::pin(read),
                kill: Box::new(NoKill),
            })
        }
    }

    /// **Risk 7 — a NO-OP recovery must not be followed by a retry.**
    ///
    /// This is the `SidecarDispatch` shape (`call.rs`'s adapter holds no `Launcher`, so its
    /// `on_fault` is `|| async { Ok(()) }`). Nothing recovered the child, so the generation is
    /// unchanged and re-attempting can only burn a second doomed round-trip — N of them with N
    /// callers in flight. Asserted on **call frames the child received**: exactly one.
    ///
    /// REVERT-CHECK: deleting the generation comparison in `call_once_or_restart` makes this 2 → RED.
    /// (Asserting on the *relaunch* count instead would be vacuous — the restart budget caps that at
    /// 5 either way. Verified; that earlier version stayed green against generation-blind code.)
    #[tokio::test]
    async fn a_noop_recovery_does_not_retry_the_same_generation() {
        let calls = Arc::new(AtomicU32::new(0));
        let launcher = CountingLauncher {
            calls: Arc::clone(&calls),
        };
        let sidecar = Sidecar::spawn(Spec::new("fake"), &launcher).await.unwrap();
        let handle = Arc::new(AsyncMutex::new(sidecar));

        let out = call_once_or_restart(&handle, "echo", "1", None, || async { Ok(()) }).await;
        assert!(
            out.is_err(),
            "the child never replies, so the call must fail"
        );

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "a no-op recovery left the SAME generation installed, so the call must not be retried"
        );
    }

    /// The complement: when the recovery genuinely REPLACED the child, the retry must still happen.
    /// A guard that blocked this would break supervised recovery on the typed path.
    #[tokio::test]
    async fn a_real_restart_is_still_retried() {
        let calls = Arc::new(AtomicU32::new(0));
        let launcher = CountingLauncher {
            calls: Arc::clone(&calls),
        };
        let sidecar = Sidecar::spawn(Spec::new("fake"), &launcher).await.unwrap();
        let handle = Arc::new(AsyncMutex::new(sidecar));

        let h = Arc::clone(&handle);
        let l = CountingLauncher {
            calls: Arc::clone(&calls),
        };
        let out = call_once_or_restart(&handle, "echo", "1", None, || async move {
            h.lock().await.restart(&l).await
        })
        .await;
        assert!(out.is_err());
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "the child was really replaced, so the one retry must proceed"
        );
    }
}
