//! Runtime **outbox taint** for an in-flight tool call (undo scope, "classification is runtime
//! transaction taint, not trusted dispatch metadata").
//!
//! The undo journal classifies an action from what its transaction *actually did*: if its path
//! reached the outbox (external motion, §6.10) it is irreversible — and that must hold even when the
//! reaching happens through a **nested** tool call from a tool that declared itself reversible. This
//! module is the mechanism the host uses to observe that, generically, at the dispatch seam.
//!
//! It is a `tokio::task_local!` boolean cell. The dispatch seam opens a scope around an outermost
//! mutating call ([`outbox_taint_scope`]); the outbox write seam marks it the moment an effect is
//! enqueued ([`mark_outbox_reached`]). Because nested host-callback calls are `.await`ed on the
//! **same task** (not spawned), they share the enclosing scope's cell — so a nested outbox reach
//! taints the *enclosing* action. That is the composition `max` rule (a reversible-declared tool
//! whose nested call reaches the outbox is irreversible as a whole) enforced for free by scoping,
//! not by a manifest field.
//!
//! Why here, in `lb-store`: both the outbox crate (which marks the taint at `write_tx`/`enqueue`)
//! and the host (which scopes + reads it at dispatch) already depend on `lb-store`, and the taint is
//! a property of the in-flight store transaction. `mark_outbox_reached` is a silent no-op when no
//! scope is open, so every non-dispatch caller of the outbox path is unaffected.

use std::cell::Cell;

/// What a tool call's in-flight transaction did to the store, observed at runtime. Both flags
/// bubble to the **enclosing** action through nested host-callback calls (same-task `.await`), so a
/// reversible-declared tool whose nested call reaches the outbox is tainted as a whole — the
/// composition `max` rule, enforced by scoping, not by a manifest field.
#[derive(Clone, Copy, Default)]
struct Taint {
    /// The transaction reached the outbox (irreversible motion, §6.10).
    outbox: bool,
    /// The transaction wrote at least one store record (so a non-capturable mutation can be told
    /// apart from a pure read — the difference between "not-undoable" and "don't journal").
    wrote: bool,
}

tokio::task_local! {
    static TAINT: Cell<Taint>;
}

fn update(f: impl FnOnce(&mut Taint)) {
    let _ = TAINT.try_with(|cell| {
        let mut t = cell.get();
        f(&mut t);
        cell.set(t);
    });
}

/// Mark the in-flight tool call's transaction as having reached the outbox (irreversible motion).
/// A no-op when no [`taint_scope`] is open on this task — so the raw outbox/`write_tx` path is
/// unaffected outside dispatch (tests, background relays, etc. that don't want classification).
pub fn mark_outbox_reached() {
    update(|t| t.outbox = true);
}

/// Mark the in-flight tool call's transaction as having written a store record. Set by the store
/// write seam so the dispatch seam can distinguish a non-capturable *mutation* (mark not-undoable)
/// from a pure read (don't journal). No-op outside a [`taint_scope`].
pub fn mark_store_written() {
    update(|t| t.wrote = true);
}

/// True if an outbox-taint scope is open AND it was marked outbox-reached. False with no scope.
pub fn outbox_was_reached() -> bool {
    TAINT.try_with(|cell| cell.get().outbox).unwrap_or(false)
}

/// True if a taint scope is open AND a store write was marked. False with no scope.
pub fn store_was_written() -> bool {
    TAINT.try_with(|cell| cell.get().wrote).unwrap_or(false)
}

/// The verdict a [`taint_scope`] returns alongside the wrapped future's output.
#[derive(Clone, Copy, Default)]
pub struct TaintVerdict {
    pub reached_outbox: bool,
    pub wrote_store: bool,
}

/// Run `fut` under a taint scope and return `(output, verdict)`.
///
/// If a scope is **already** open on this task (a nested/re-entrant call), `fut` runs under the
/// existing cell — nested writes/outbox reaches bubble to the *enclosing* action (max-composition)
/// — and the verdict reflects the enclosing cell at `fut`'s completion. Only an outermost call
/// installs a fresh cell. This is the seam the host wraps a dispatched tool call in.
pub async fn taint_scope<F, T>(fut: F) -> (T, TaintVerdict)
where
    F: std::future::Future<Output = T>,
{
    if TAINT.try_with(|_| ()).is_ok() {
        let out = fut.await;
        let t = TAINT.try_with(|c| c.get()).unwrap_or_default();
        (
            out,
            TaintVerdict {
                reached_outbox: t.outbox,
                wrote_store: t.wrote,
            },
        )
    } else {
        TAINT
            .scope(Cell::new(Taint::default()), async move {
                let out = fut.await;
                let t = TAINT.try_with(|c| c.get()).unwrap_or_default();
                (
                    out,
                    TaintVerdict {
                        reached_outbox: t.outbox,
                        wrote_store: t.wrote,
                    },
                )
            })
            .await
    }
}
