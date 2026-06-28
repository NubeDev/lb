//! `AiMeter` — the per-run AI budget. **Lifted verbatim from rubix-cube** (`rules/ai.rs`,
//! MIT/Apache-2.0): a `for`-loop of `ai.complete` cannot run up an unbounded bill. Call + token caps,
//! atomic so concurrent verb calls within a run still bound. A rejected call is NOT counted (the
//! `fetch_sub` rollback). This is one of the two AI invariants the scope keeps exactly (the other is
//! the nsql fence, enforced in the `ai` verb by re-validating proposed SQL through [`crate::grid`]).

use std::sync::atomic::{AtomicU32, Ordering};

/// The budget for one run. Defaults from node config (the `env::rules::AI_*` knobs).
#[derive(Debug)]
pub struct AiMeter {
    calls: AtomicU32,
    tokens: AtomicU32,
    max_calls: u32,
    max_tokens: u32,
}

impl AiMeter {
    pub fn new(max_calls: u32, max_tokens: u32) -> Self {
        Self {
            calls: AtomicU32::new(0),
            tokens: AtomicU32::new(0),
            max_calls,
            max_tokens,
        }
    }

    /// Charge one call. Rolls back and errors if it would exceed `max_calls` (a rejected call is not
    /// counted — port the `fetch_sub`).
    pub fn charge_call(&self) -> Result<(), String> {
        let prev = self.calls.fetch_add(1, Ordering::SeqCst);
        if prev >= self.max_calls {
            self.calls.fetch_sub(1, Ordering::SeqCst);
            return Err(format!(
                "AI budget exceeded: at most {} AI calls per run",
                self.max_calls
            ));
        }
        Ok(())
    }

    /// Charge tokens. Errors if the running total exceeds `max_tokens`.
    pub fn charge_tokens(&self, tokens: u32) -> Result<(), String> {
        let total = self.tokens.fetch_add(tokens, Ordering::SeqCst) + tokens;
        if total > self.max_tokens {
            return Err(format!(
                "AI budget exceeded: at most {} tokens per run",
                self.max_tokens
            ));
        }
        Ok(())
    }

    pub fn calls_used(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }

    pub fn tokens_used(&self) -> u32 {
        self.tokens.load(Ordering::SeqCst)
    }
}
