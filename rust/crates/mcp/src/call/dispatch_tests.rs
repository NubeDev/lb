//! Tests for [`super::dispatch`] — the local/remote dispatch seam.
//!
//! Split out of `dispatch.rs` to keep that file under the FILE-LAYOUT 400-line limit; it is the
//! same `#[cfg(test)] mod tests` as before, reached via `#[path]` from its parent, so `super::*`
//! still resolves to the dispatch module.

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
            reentrancy::holding(0xA_usize, dispatch(&b2, &bus2, "ws", "b.tool", "{}", None)).await
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

/// A **multiplexing** target (a native sidecar: a separate process that serves many calls at
/// once) must NOT be serialised by the per-instance mutex. The mutex exists for a wasm
/// instance, which genuinely cannot run two calls; applying it to a sidecar caps a whole
/// extension at concurrency 1 node-wide however well the child multiplexes underneath.
///
/// Measured before the fix (ext-esr, live node, database confirmed idle): a 20 ms LOCAL store
/// read took **8.96 s** while one slow verb was in flight, and three cheap calls issued in
/// parallel behind it all released at the SAME instant (9.78/9.78/9.79 s) — a mutex signature,
/// not a queue.
///
/// Revert-checked: with `is_multiplexing` forced to `false` this hangs until the timeout and
/// fails, because the second call waits for the first to release the lock.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_multiplexing_target_is_not_serialised_by_the_instance_lock() {
    /// Stands in for a native sidecar: blocks on its first call, answers instantly after, and
    /// declares that it multiplexes.
    struct Multiplexing {
        started: Arc<Notify>,
        release: Arc<Notify>,
        blocked_once: Arc<AtomicBool>,
    }

    #[async_trait]
    impl LocalDispatch for Multiplexing {
        fn is_multiplexing(&self) -> bool {
            true
        }

        /// The detached path: everything it needs is cloned out here, under the lock, so the
        /// returned future owns its state and the host can await it unlocked.
        fn call_tool_detached(
            &self,
            _ws: &str,
            _tool: &str,
            _input_json: &str,
            _ctx: Option<CallContext>,
        ) -> Option<lb_runtime::BoxFuture<'static, Result<String, RuntimeError>>> {
            let (started, release, blocked_once) = (
                self.started.clone(),
                self.release.clone(),
                self.blocked_once.clone(),
            );
            Some(Box::pin(async move {
                if !blocked_once.swap(true, Ordering::SeqCst) {
                    started.notify_one();
                    release.notified().await;
                }
                Ok("{}".into())
            }))
        }

        async fn call_tool(
            &mut self,
            _ws: &str,
            _tool: &str,
            _input_json: &str,
            _ctx: Option<CallContext>,
        ) -> Result<String, RuntimeError> {
            unreachable!("dispatch must take the detached path for a multiplexing target")
        }
    }

    let bus = Bus::peer().await.unwrap();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let target = Arc::new(Target::Local(hosted(Multiplexing {
        started: started.clone(),
        release: release.clone(),
        blocked_once: Arc::new(AtomicBool::new(false)),
    })));

    // The slow call, left in flight.
    let slow = {
        let (target, bus) = (Arc::clone(&target), bus.clone());
        tokio::spawn(async move {
            reentrancy::in_scope(dispatch(&target, &bus, "ws", "x.slow", "{}", None)).await
        })
    };
    started.notified().await; // it is genuinely inside the handler now

    // The cheap call beside it must answer WITHOUT waiting for the slow one.
    let cheap = tokio::time::timeout(
        Duration::from_secs(5),
        reentrancy::in_scope(dispatch(&target, &bus, "ws", "x.cheap", "{}", None)),
    )
    .await;

    assert!(
        cheap.is_ok(),
        "a cheap call was BLOCKED behind a slow one on a multiplexing target — the per-instance \
         mutex is still serialising a native sidecar (concurrency 1 per extension, node-wide)"
    );
    assert!(cheap.unwrap().is_ok(), "the cheap call itself must succeed");

    release.notify_one();
    assert!(
        slow.await.unwrap().is_ok(),
        "the slow call must still complete"
    );
}

/// The wasm case is unchanged: a non-multiplexing instance is STILL serialised, because one
/// wasm instance really cannot run two calls at once. The fix must not widen concurrency for
/// the target the mutex was built for.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_multiplexing_target_is_still_serialised() {
    let bus = Bus::peer().await.unwrap();
    let started = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let target = Arc::new(Target::Local(hosted(Blocking {
        started: started.clone(),
        release: release.clone(),
        blocked_once: Arc::new(AtomicBool::new(false)),
    })));

    let slow = {
        let (target, bus) = (Arc::clone(&target), bus.clone());
        tokio::spawn(async move {
            reentrancy::in_scope(dispatch(&target, &bus, "ws", "x.slow", "{}", None)).await
        })
    };
    started.notified().await;

    // This MUST time out: the wasm instance is exclusive, so the second call waits.
    let cheap = tokio::time::timeout(
        Duration::from_millis(300),
        reentrancy::in_scope(dispatch(&target, &bus, "ws", "x.cheap", "{}", None)),
    )
    .await;
    assert!(
        cheap.is_err(),
        "a wasm instance must STILL be serialised — one instance cannot run two calls"
    );

    release.notify_one();
    assert!(slow.await.unwrap().is_ok());
}
