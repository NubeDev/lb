//! `AgentRuleModel` — the [`RuleModel`] adapter over the host's [`ModelAccess`] seam
//! (rules-ai-wiring-scope). This is the one new file the scope asks for: it binds the rule engine's
//! model boundary to the SAME model the agent core uses (the node's `default` runtime model), so a
//! rule's `ai.*` reaches a real model instead of the hardcoded `DisabledModel`.
//!
//! **Single-turn, no tools — on purpose.** A rule's `ai.*` is a bounded, metered, sandboxed
//! completion, NOT an agent run: the adapter calls `ModelAccess::turn` with an EMPTY tools list and
//! reads the first turn's content. It never runs the agent loop, never gives the model an independent
//! tool channel (a rule's data/emit power comes from the *rule verbs*, gated by the cage + caps). If a
//! rule genuinely needs the agent loop it emits `agent.invoke` — a separate seam (scope non-goal).
//!
//! **The nsql fence is untouched.** `propose_sql` returns the model's SQL *string*; the proposed SQL
//! still flows back through `DataSeam::collect`'s validator + `caps::check` in the `ai` verb before it
//! can run (rules-engine-scope). This adapter fills the model — it does not touch the fence, the meter,
//! or the cage.

use std::sync::Arc;

use tokio::runtime::Handle;

use super::seam::RuleModel;
use crate::agent::{AllowedTool, ErasedModel};

/// The model seam for a rule run, over the node's [`ModelAccess`]. Holds the erased model handle (the
/// SAME `Arc` the in-house `default` runtime runs — one model, not a copy), the pinned workspace, a
/// `block_on` handle (the rule engine runs on a blocking thread and calls this SYNCHRONOUS seam), and a
/// per-run idempotency prefix so a re-run replays cleanly through the gateway's turn cache.
pub struct AgentRuleModel {
    model: Arc<dyn ErasedModel>,
    ws: String,
    handle: Handle,
    /// A per-run key prefix (`(ws, rule, run-ts)`-derived). Each `ai.*` call within the run appends
    /// its verb + a monotonic index so two calls in the same run don't collide in the gateway cache.
    idem_prefix: String,
    seq: std::sync::atomic::AtomicU32,
}

impl AgentRuleModel {
    pub fn new(
        model: Arc<dyn ErasedModel>,
        ws: impl Into<String>,
        idem_prefix: impl Into<String>,
    ) -> Self {
        Self {
            model,
            ws: ws.into(),
            handle: Handle::current(),
            idem_prefix: idem_prefix.into(),
            seq: std::sync::atomic::AtomicU32::new(0),
        }
    }

    /// One bounded model turn with NO tools. Blocks the rule's blocking thread on the host's async
    /// `ModelAccess::turn` (never called on the async worker — the rule engine already runs under
    /// `spawn_blocking`). Returns the turn's content, or a clear error if the model returned only tool
    /// calls with no completion (a rule has no loop to run them — surface it, never hang).
    fn turn(&self, prompt: &str) -> Result<(String, u32), String> {
        let n = self.seq.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let key = format!("{}:{n}", self.idem_prefix);
        let model = self.model.clone();
        let ws = self.ws.clone();
        let messages = vec![("user".to_string(), prompt.to_string())];
        let tools: Vec<AllowedTool> = Vec::new();
        let turn = self
            .handle
            .block_on(async move { model.turn_boxed(&ws, &messages, &tools, &[], &key).await });
        if turn.content.trim().is_empty() && !turn.calls.is_empty() {
            // The model asked to call tools but a rule's `ai.*` is single-turn with no tool channel —
            // there is nothing to run and no next turn. Surface a clear error rather than loop/hang.
            return Err(
                "AI returned only tool calls, no completion — a rule's ai.* is single-turn (no tools)"
                    .into(),
            );
        }
        // `Turn` carries no token count today (the gateway's `AiResponse` tokens are dropped at the
        // `ModelAccess` altitude). Approximate from the completion length so the budget meter still
        // bites proportionally (rules-engine-scope keeps the meter). KNOWN GAP (scope "Budget under a
        // real model"): when `Turn` grows a real token field, return it here instead of this estimate —
        // else the budget is only an approximation. Never a placeholder that reports zero.
        let tokens = estimate_tokens(&turn.content);
        Ok((turn.content, tokens))
    }
}

impl RuleModel for AgentRuleModel {
    fn complete(&self, prompt: &str) -> Result<(String, u32), String> {
        self.turn(prompt)
    }

    fn propose_sql(&self, question: &str, schema_hint: &str) -> Result<String, String> {
        // One turn with the nsql prompt (schema hint + "propose read-only SQL"). The returned SQL is
        // re-validated through `DataSeam::collect` in the `ai` verb (THE FENCE) before it runs.
        let prompt = format!(
            "You translate a question into ONE read-only SQL SELECT over the given schema. Reply with \
             ONLY the SQL, no prose, no code fence.\n\nSchema: {schema_hint}\n\nQuestion: {question}"
        );
        let (sql, _tokens) = self.turn(&prompt)?;
        Ok(sql.trim().to_string())
    }
}

/// A conservative token estimate from a completion's byte length (~4 chars/token, min 1). Used only
/// because `Turn` does not yet surface the provider's real token count — see the KNOWN GAP above.
fn estimate_tokens(content: &str) -> u32 {
    ((content.len() as u32) / 4).max(1)
}
