//! A deterministic, scripted provider for tests (testing §3: mock the true external — the model
//! provider HTTP — never the store/bus). The mock answers turns from a pre-loaded script: turn `i`
//! returns `script[i]`, so a test drives the agent's loop down an exact, repeatable path with no
//! network and no randomness.
//!
//! This is the *only* thing mocked in the S5 slice. The real provider adapter is an
//! [`AiGateway`](crate::AiGateway) swap behind the same [`Provider`] trait (ai-gateway scope).

use std::sync::Mutex;

use crate::fault::ProviderFault;
use crate::provider::Provider;
use crate::request::AiRequest;
use crate::response::AiResponse;

/// A provider that replays a fixed script of turns in order. Past the end it returns a terminal
/// `stop` (so an over-long loop ends rather than panicking).
///
/// The script's failure arm (agent-loop-hardening slice D) lets a test drive every fault lane —
/// a 429 with `Retry-After`, a context overflow, an auth failure — through the REAL gateway + loop
/// (rule 9: this is the one sanctioned fake, standing in only for the provider HTTP).
pub struct MockProvider {
    script: Vec<Result<AiResponse, ProviderFault>>,
    next: Mutex<usize>,
}

impl MockProvider {
    /// Build a mock that answers turn `i` with `script[i]` (all-success — the common test shape).
    pub fn new(script: Vec<AiResponse>) -> Self {
        Self::scripted(script.into_iter().map(Ok).collect())
    }

    /// Build a mock whose script mixes completions and typed faults — turn `i` yields `script[i]`
    /// verbatim, so a test drives the loop's retry/compact/terminal lanes deterministically.
    pub fn scripted(script: Vec<Result<AiResponse, ProviderFault>>) -> Self {
        Self {
            script,
            next: Mutex::new(0),
        }
    }
}

impl Provider for MockProvider {
    async fn complete(&self, _req: &AiRequest) -> Result<AiResponse, ProviderFault> {
        let mut next = self.next.lock().expect("mock lock");
        let i = *next;
        *next += 1;
        self.script
            .get(i)
            .cloned()
            .unwrap_or_else(|| Ok(AiResponse::stop("(mock: script exhausted)", 0)))
    }
}
