//! The relay's delivery seam — the `Target` an outbox effect is delivered to (outbox scope: "the
//! relay delivers to a `Target` trait; a real adapter rides behind the same trait"). Host-owned,
//! exactly like the agent's `ModelAccess`: the host defines the trait and the relay calls only it;
//! a delivery adapter or the test supplies the impl. **Provider-free by construction** (rule 10): the
//! trait names only `lb_outbox::Effect`, never any extension — a real adapter is an extension that
//! implements it, reached as opaque data, so swapping one adapter for another needs zero change here.
//!
//! Lives in `outbox/` (relocated from the retired `workflow/` service, rules-workflow-convergence
//! scope): it is an outbox delivery primitive, consumed by the outbox-sink flow node's relay reactor,
//! the reminders/approval reactors, and native-tier extensions (all through `lb_host::Target`).
//!
//! Keeping delivery behind a trait is what makes the relay testable deterministically (the test
//! target records calls and can be told to fail-then-succeed — the only external mocked, testing §3)
//! and what lets new targets be extension-provided without touching the relay (§6.10 finding).

use std::future::Future;

use lb_outbox::Effect;

/// A delivery sink for outbox effects. `deliver` performs the external effect (whatever the target's
/// adapter does with it) and returns whether the target acknowledged it. An `Err`/`false` is a transient
/// failure — the relay leaves the effect schedulable and retries next pass (at-least-once). The
/// implementation MUST dedup on `effect.idempotency_key`, so an at-least-once re-delivery is a
/// no-op on the outside world (the at-least-once → effectively-once bridge, outbox scope).
pub trait Target {
    /// Attempt to deliver `effect`. `Ok(())` = acknowledged (the relay marks it delivered);
    /// `Err(reason)` = failed (the relay marks it failed; it re-delivers next pass).
    fn deliver(&self, effect: &Effect) -> impl Future<Output = Result<(), String>> + Send;
}
