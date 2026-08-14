//! The same-instance re-entrancy guard (mcp scope; corrects `host-callback-scope.md` Open Q1,
//! whose resolution this restores — see `docs/debugging/mcp/gauge-panel-loses-extension-busy-race.md`).
//!
//! There is one `Mutex` per loaded extension instance ([`Hosted::instance`](crate::registry::Hosted)).
//! Awaiting it deadlocks in exactly ONE case: a call chain re-enters the SAME instance its own
//! ancestor already holds (a guest's `host.call-tool` calling back into itself). A nested call
//! that targets a DIFFERENT instance cannot deadlock this chain — nothing in the chain holds
//! that lock — so it can safely `.await` it, exactly like a top-level call, and simply serialize
//! behind whoever else is using it.
//!
//! This module tracks, per task, which instance pointers the in-flight call chain currently
//! holds — the ONLY question [`dispatch`](super::dispatch::dispatch) needs answered to pick
//! `try_lock` (fail fast, self-re-entry) vs `lock().await` (wait, everything else). It is a
//! `tokio::task_local!` set, mirroring `lb_store::taint`'s scope discipline: nested host-callback
//! calls are `.await`ed on the SAME task (never spawned), so they share the enclosing scope's
//! cell for free — no identity has to ride the call arguments.

use std::cell::RefCell;
use std::future::Future;

tokio::task_local! {
    static HELD: RefCell<Vec<usize>>;
}

/// True if `ptr` — an instance's identity, `Arc::as_ptr(&hosted.instance) as *const () as usize`
/// — is already held by an ancestor call in this task's chain. The one case a nested dispatch
/// must not block on.
pub fn is_held(ptr: usize) -> bool {
    HELD.try_with(|held| held.borrow().contains(&ptr))
        .unwrap_or(false)
}

/// Run `fut` with a `HELD` scope open on this task. A no-op re-entry if one is already open (the
/// nested-call case — it shares the enclosing scope, which is what makes [`is_held`] see an
/// ancestor's instance); otherwise installs a fresh, empty one (the outermost dispatch on this
/// task). Every call funnels through `dispatch`, so wrapping it there is the one place that
/// establishes the invariant — no outer call site needs to know this exists.
pub async fn in_scope<F: Future>(fut: F) -> F::Output {
    if HELD.try_with(|_| ()).is_ok() {
        fut.await
    } else {
        HELD.scope(RefCell::new(Vec::new()), fut).await
    }
}

/// Mark `ptr` held for the duration of `fut` — wrap the locked instance's own call in this so
/// the pointer is visible to any nested dispatch for exactly as long as the lock is actually
/// held. Removal runs on drop, so an early return/error still clears it.
pub async fn holding<F: Future>(ptr: usize, fut: F) -> F::Output {
    let _guard = HeldGuard::install(ptr);
    fut.await
}

struct HeldGuard {
    ptr: usize,
}

impl HeldGuard {
    fn install(ptr: usize) -> Self {
        let _ = HELD.try_with(|held| held.borrow_mut().push(ptr));
        Self { ptr }
    }
}

impl Drop for HeldGuard {
    fn drop(&mut self) {
        let ptr = self.ptr;
        let _ = HELD.try_with(|held| {
            let mut held = held.borrow_mut();
            if let Some(pos) = held.iter().rposition(|p| *p == ptr) {
                held.remove(pos);
            }
        });
    }
}
