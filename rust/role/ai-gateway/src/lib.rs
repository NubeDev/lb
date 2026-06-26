//! The shared **AI gateway** — the swappable, stateless model-access service behind a stable
//! contract (README §6.14, ai-gateway scope). It is *not* an agent and does *not* run the tool-call
//! loop: a request goes in, a completion (with any proposed tool calls) comes back, and the
//! **caller** (the central agent §6.16, agent scope) runs the loop. Keeping the loop out of the
//! gateway is what lets the implementation be swapped (mock now; a real OpenAI-compatible / SpiceAI
//! / local adapter later — one contract, many implementations).
//!
//! Role-only: this is a Tier-2-style sidecar the node talks to, not a core crate. The core depends
//! on the **contract** ([`AiRequest`]/[`AiResponse`]/[`Provider`]), never the implementation
//! (ai-gateway scope, symmetric nodes — placement is config).
//!
//! S5 ships: the contract types, the [`Provider`] seam, a deterministic [`MockProvider`] for tests
//! (the only thing mocked — testing §3), and the replay-safe idempotency cache on [`AiGateway`].
//! Streaming, embeddings, real adapters, audit, and secrets-backed keys are deferred (scope).
//!
//! One responsibility per file (FILE-LAYOUT §3): the request, the response, the provider seam, the
//! mock, the gateway.

mod bridge;
mod gateway;
mod mock;
mod provider;
mod request;
mod response;

pub use gateway::AiGateway;
pub use mock::MockProvider;
pub use provider::Provider;
pub use request::{AiRequest, Message, ToolSchema};
pub use response::{AiResponse, FinishReason, ToolCall, ToolResult};
