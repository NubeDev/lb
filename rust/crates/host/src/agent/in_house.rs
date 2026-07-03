//! The **in-house** [`AgentRuntime`] — the always-registered default (runtime-seam #1). It is the
//! existing bounded tool-call loop (`run.rs`) presented behind the host-owned seam, so selecting a
//! runtime is a registry lookup and the default is *just another entry* (one fewer entry on a
//! feature-off node, never a code branch).
//!
//! It holds the node's [`ModelAccess`] erased behind [`ErasedModel`] (so it can be a trait object),
//! and its `run` re-derives the session actor and calls [`run_session`](super::run::run_session) —
//! the **same** loop, no second implementation. This is the "blanket-impl the in-house loop as the
//! always-registered default" the scope asks for.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use super::error::AgentError;
use super::run::run_session;
use super::runtime::{AgentRuntime, ErasedModel, ModelHandle, RunContext};
use crate::boot::Node;

/// The default runtime id — absent/`"default"`/unknown-with-fallback all resolve here.
pub const DEFAULT_RUNTIME: &str = "default";

/// The in-house loop as a registry entry. Stores the erased model so it is a `Box<dyn AgentRuntime>`.
pub struct InHouseRuntime {
    model: Arc<dyn ErasedModel>,
}

impl InHouseRuntime {
    /// Register the in-house loop over `model` (any [`ModelAccess`](super::model_access::ModelAccess),
    /// erased). Called once when the node builds its registry.
    pub fn new(model: Arc<dyn ErasedModel>) -> Self {
        Self { model }
    }
}

impl AgentRuntime for InHouseRuntime {
    fn id(&self) -> &str {
        DEFAULT_RUNTIME
    }

    fn run<'a>(
        &'a self,
        node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // Round-trip the erased model back into a `ModelAccess` and drive the SAME loop. No new
            // path: this is `run.rs` verbatim, reached through the seam.
            let model = ModelHandle(self.model.clone());
            run_session(
                node,
                &model,
                ctx.caller,
                ctx.agent_caps,
                ctx.ws,
                ctx.job_id,
                ctx.goal,
                ctx.tools,
                ctx.ts,
            )
            .await
        })
    }
}
