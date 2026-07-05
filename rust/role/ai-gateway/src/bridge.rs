//! The bridge: adapt this gateway's [`AiGateway`] to the host-owned `lb_host::ModelAccess` trait,
//! so the host's agent loop reaches a model **without build-depending on this role crate** (roles
//! depend on host, not the reverse — symmetric layering). The host depends only on the trait it
//! owns; this crate, which already depends on host, supplies the impl.
//!
//! The bridge translates the host's altitude (messages + allowed tools + prior outcomes + an
//! idempotency key) to/from the gateway contract ([`AiRequest`]/[`AiResponse`]). It runs **no
//! loop** — `complete` answers one turn; the host loop runs the proposed calls (agent scope).

use lb_host::{AllowedTool, CallOutcome, ModelAccess, ProposedCall, Turn};

use crate::gateway::AiGateway;
use crate::provider::Provider;
use crate::request::{AiRequest, Message, ToolSchema};
use crate::response::{FinishReason, ToolResult};

impl<P: Provider> ModelAccess for AiGateway<P> {
    async fn turn(
        &self,
        ws: &str,
        messages: &[(String, String)],
        tools: &[AllowedTool],
        prior: &[CallOutcome],
        idempotency_key: &str,
    ) -> Turn {
        let mut req = AiRequest::new(ws, idempotency_key);
        req.messages = messages
            .iter()
            .map(|(role, content)| Message::new(role, content))
            .collect();
        req.tools = tools
            .iter()
            .map(|t| ToolSchema {
                name: t.name.clone(),
                description: t.description.clone(),
                parameters: t.input_schema.clone(),
            })
            .collect();
        req.prior_results = prior
            .iter()
            .map(|o| ToolResult {
                id: o.id.clone(),
                name: o.name.clone(),
                input: o.input.clone(),
                ok: o.ok.clone(),
                error: o.error.clone(),
            })
            .collect();

        let resp = self.complete(&req).await;

        Turn {
            content: resp.content,
            calls: resp
                .tool_calls
                .into_iter()
                .map(|c| ProposedCall {
                    id: c.id,
                    name: c.name,
                    input: c.input,
                })
                .collect(),
            done: resp.finish_reason == FinishReason::Stop,
        }
    }
}
