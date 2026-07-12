//! The **unconfigured in-house model** — the placeholder [`ModelAccess`] the node's runtime registry
//! (`boot`) binds to the always-present `default` runtime when no real model provider has been wired
//! yet. This is the honest stand-in for the "`agent_invoke` needs a real model provider" gap
//! (`STATUS.md`): a node can still hold a valid [`RuntimeRegistry`](super::registry::RuntimeRegistry)
//! (so the resolve invariant "absent → default" holds and EXTERNAL runtimes are selectable), while the
//! in-house `default` returns a clear, non-secret "not configured" answer instead of pretending to run.
//!
//! It is **not** a fake backend (rule 9): it runs no model and imitates none — it is the truthful empty
//! state of an un-provisioned in-house loop. The moment a real `ModelAccess` (the AI gateway) is wired
//! at boot, it replaces this via `Node::install_runtimes` and nothing else changes (the seam is the
//! registry, not this type).

use std::future::Future;

use super::model_access::{AllowedTool, CallOutcome, ModelAccess, Turn, TurnError};

/// The message the in-house default returns on a node with no model provider configured. Deliberately
/// explicit so a caller (or a channel `agent_result`) never mistakes it for a real answer.
pub const UNCONFIGURED_ANSWER: &str =
    "no in-house model is configured on this node; select an external runtime (e.g. \
     open-interpreter-default) or wire a model provider";

/// A [`ModelAccess`] that performs no turn — it immediately finishes with [`UNCONFIGURED_ANSWER`]. The
/// blanket `impl ErasedModel for M: ModelAccess` makes it storable in the registry as the default.
pub struct UnconfiguredModel;

impl ModelAccess for UnconfiguredModel {
    fn turn(
        &self,
        _ws: &str,
        _messages: &[(String, String)],
        _tools: &[AllowedTool],
        _prior: &[CallOutcome],
        _idempotency_key: &str,
    ) -> impl Future<Output = Result<Turn, TurnError>> + Send {
        async {
            Ok(Turn {
                content: UNCONFIGURED_ANSWER.to_string(),
                calls: vec![],
                done: true,
            })
        }
    }

    /// This is the placeholder, not a real provider — so a non-agent caller (the rules engine)
    /// keeps the honest "AI not configured" path rather than returning [`UNCONFIGURED_ANSWER`] as
    /// if it were a model answer.
    fn is_configured(&self) -> bool {
        false
    }
}
