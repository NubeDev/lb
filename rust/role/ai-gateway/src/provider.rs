//! The provider seam — the one trait every model backend implements (ai-gateway scope: "one
//! gateway contract, many implementations"). The real OpenAI-compatible / SpiceAI / local-model
//! adapters slot in here behind the same `complete`; S5 ships only the [`MockProvider`](crate::mock)
//! for deterministic tests (testing §3 — mock only the true external, the provider HTTP).
//!
//! A provider does **model access only**: given the request, return one turn's [`AiResponse`]. It
//! never runs tools, never holds the loop, never sees the store — that is the agent's job.
//!
//! Native async-fn-in-trait (no `async_trait` dep): the gateway is generic over `P: Provider`, so
//! the trait is never used as `dyn` — keeping the contract dependency-free.

use std::future::Future;

use crate::request::AiRequest;
use crate::response::AiResponse;

/// A model backend. `complete` answers one turn. `Send + Sync` so the gateway holding it can be
/// shared across the node's async tasks.
pub trait Provider: Send + Sync {
    /// Answer one turn for `req`. Real adapters do network IO here; the mock resolves immediately.
    fn complete(&self, req: &AiRequest) -> impl Future<Output = AiResponse> + Send;
}
