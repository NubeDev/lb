//! [`RouterTarget`] — the generic composite `Target` that dispatches an effect to the registered
//! delivery adapter for its `effect.target` string (release scope, gap 1: relay boot wiring). The
//! relay loop takes ONE `Target`; a booted node has several adapters (email, push, …), so the boot
//! seam registers each under its target string here and hands the router to
//! [`spawn_relay_reactors`](super::spawn_relay_reactors).
//!
//! Rule 10 by construction: the target string is **opaque routing data** — the router never names
//! an adapter or branches on a specific id; a product host registers whatever adapters it wants
//! under whatever strings its effects use. An effect whose target has no registered route fails
//! the pass (`Err`), so it retries and eventually dead-letters with a clear reason instead of
//! silently vanishing — the outbox's normal poison-message posture.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lb_outbox::Effect;

use super::delivery_error::DeliveryError;
use super::target::Target;

/// A dyn-compatible twin of [`Target`] (whose `deliver` returns `impl Future`, so `dyn Target`
/// itself is not usable). Blanket-implemented for every `Target`; the router stores these.
pub trait DynTarget: Send + Sync {
    fn deliver_dyn<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeliveryError>> + Send + 'a>>;
}

impl<T> DynTarget for T
where
    T: Target + Send + Sync,
{
    fn deliver_dyn<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> Pin<Box<dyn Future<Output = Result<(), DeliveryError>> + Send + 'a>> {
        Box::pin(self.deliver(effect))
    }
}

/// The composite target: `effect.target` string → registered adapter. Built once at boot.
#[derive(Default)]
pub struct RouterTarget {
    routes: HashMap<String, Arc<dyn DynTarget>>,
}

impl RouterTarget {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `target` (e.g. the email/push adapter) under the opaque `target_str`. Builder-style.
    pub fn route(mut self, target_str: &str, target: impl Target + Send + Sync + 'static) -> Self {
        self.routes.insert(target_str.to_string(), Arc::new(target));
        self
    }

    /// Register an ALREADY-ERASED adapter under `target_str`. The twin of [`route`](Self::route) for
    /// a caller that cannot name the concrete type — notably an embedder handing targets in through
    /// `BootConfig`, where the whole point is that the core does not know what they are.
    pub fn route_dyn(mut self, target_str: &str, target: Arc<dyn DynTarget>) -> Self {
        self.routes.insert(target_str.to_string(), target);
        self
    }
}

impl Target for RouterTarget {
    // Explicit `impl Future` is load-bearing here (trait-object-safe, explicit lifetime capture).
    #[allow(clippy::manual_async_fn)]
    fn deliver(
        &self,
        effect: &Effect,
    ) -> impl std::future::Future<Output = Result<(), DeliveryError>> + Send {
        async move {
            match self.routes.get(&effect.target) {
                Some(t) => t.deliver_dyn(effect).await,
                // No route ⇒ TRANSIENT, deliberately, even though boot's route table never grows: an
                // effect for a *sidecar-driven* target (`relay_ops`: the driver pulls its own effects
                // via `outbox.due` and marks them itself) has no in-process route by design. Failing it
                // permanently on the first tick would kill effects that were never this router's to
                // deliver. So it keeps the pre-existing posture — retry, then dead-letter with a reason.
                None => Err(DeliveryError::transient(format!(
                    "no delivery adapter registered for target '{}'",
                    effect.target
                ))),
            }
        }
    }
}
