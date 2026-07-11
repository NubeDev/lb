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

use lb_outbox::Effect;

use super::target::Target;

/// A dyn-compatible twin of [`Target`] (whose `deliver` returns `impl Future`, so `dyn Target`
/// itself is not usable). Blanket-implemented for every `Target`; the router stores these.
pub trait DynTarget: Send + Sync {
    fn deliver_dyn<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>>;
}

impl<T> DynTarget for T
where
    T: Target + Send + Sync,
{
    fn deliver_dyn<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + 'a>> {
        Box::pin(self.deliver(effect))
    }
}

/// The composite target: `effect.target` string → registered adapter. Built once at boot.
#[derive(Default)]
pub struct RouterTarget {
    routes: HashMap<String, Box<dyn DynTarget>>,
}

impl RouterTarget {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register `target` (e.g. the email/push adapter) under the opaque `target_str`. Builder-style.
    pub fn route(mut self, target_str: &str, target: impl Target + Send + Sync + 'static) -> Self {
        self.routes.insert(target_str.to_string(), Box::new(target));
        self
    }
}

impl Target for RouterTarget {
    fn deliver(
        &self,
        effect: &Effect,
    ) -> impl std::future::Future<Output = Result<(), String>> + Send {
        async move {
            match self.routes.get(&effect.target) {
                Some(t) => t.deliver_dyn(effect).await,
                None => Err(format!(
                    "no delivery adapter registered for target '{}'",
                    effect.target
                )),
            }
        }
    }
}
