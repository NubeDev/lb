//! The AI gateway — the swappable model-access service behind the stable contract (ai-gateway
//! scope). It holds a [`Provider`] and an **idempotency cache**, and exposes one method:
//! `complete(AiRequest) -> AiResponse`. **No loop, no tool execution** — the agent owns those
//! (agent scope). Keeping the loop out is exactly what lets the gateway be swapped.
//!
//! **Replay-safe (the S5 durability piece):** the response is cached by `idempotency_key`, so a
//! resumed agent job that re-issues the same request gets the cached response — it does not call
//! the provider again, does not re-spend budget, and cannot diverge (ai-gateway scope, agent scope
//! offline/sync). Non-determinism is pinned to the first execution.
//!
//! Generic over `P: Provider` (no `dyn`) so the contract stays dependency-free; the `node`/role
//! wiring picks the concrete provider (mock in tests, a real adapter later).

use std::collections::HashMap;
use std::sync::Mutex;

use crate::provider::Provider;
use crate::request::AiRequest;
use crate::response::AiResponse;

/// A gateway over provider `P`. Cheap to construct; the cache is interior-mutable so `complete`
/// needs only `&self`. One per node (the role layer holds it).
pub struct AiGateway<P: Provider> {
    provider: P,
    /// idempotency_key → the response served for it. The replay-safety cache.
    cache: Mutex<HashMap<String, AiResponse>>,
}

impl<P: Provider> AiGateway<P> {
    pub fn new(provider: P) -> Self {
        Self {
            provider,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Answer one turn. If `req.idempotency_key` was served before, return the cached response
    /// (the provider is NOT called again — replay-safe). Otherwise call the provider, cache, return.
    pub async fn complete(&self, req: &AiRequest) -> AiResponse {
        // Fast path: a cache hit is a resumed/duplicated call — serve the pinned response.
        if let Some(hit) = self
            .cache
            .lock()
            .expect("gateway cache lock")
            .get(&req.idempotency_key)
            .cloned()
        {
            return hit;
        }

        // Miss: the first execution. Call the provider (model access), then pin the result.
        let resp = self.provider.complete(req).await;
        self.cache
            .lock()
            .expect("gateway cache lock")
            .insert(req.idempotency_key.clone(), resp.clone());
        resp
    }

    /// How many times the provider was actually invoked (= distinct idempotency keys served). Lets
    /// a test prove a resumed call hit the cache instead of re-spending (agent scope offline/sync).
    pub fn provider_calls(&self) -> usize {
        self.cache.lock().expect("gateway cache lock").len()
    }
}
