//! The **runtime registry** (runtime-seam #1) — a small map from a `runtime` id to a
//! [`AgentRuntime`] trait object. The in-house loop is **always** registered as `"default"`; the
//! external `AcpRuntime` (sub-scope #2) registers itself **only when the `external-agent` cargo
//! feature is on** (the node's registration hook adds the entry). A feature-off node simply has one
//! fewer entry — no call site enumerates runtime kinds (registry, not a `match`).
//!
//! **Resolution (the runtime-seam open-question, decided):**
//! - **absent** `runtime` → the **default** (in-house loop);
//! - an **explicitly-named** id that exists → that runtime;
//! - an **explicitly-named unknown** id → **error** (`AgentError::BadInput`) — a caller that asked
//!   for a specific runtime must not be silently downgraded to a different engine.
//!
//! This is the whole of #1's selection surface. No new caller capability: choosing a runtime is an
//! argument resolved here, not a grant (invoking stays gated by `mcp:agent.invoke:call`).

use std::collections::HashMap;
use std::sync::Arc;

use super::error::AgentError;
use super::in_house::{InHouseRuntime, DEFAULT_RUNTIME};
use super::runtime::{AgentRuntime, ErasedModel};

/// The node's configured runtimes, keyed by id. Cheap to clone-share (`Arc` entries) into the invoke
/// path and the `agent.runtimes` read verb (#5, TODO).
#[derive(Clone)]
pub struct RuntimeRegistry {
    entries: HashMap<String, Arc<dyn AgentRuntime>>,
    default_id: String,
}

impl RuntimeRegistry {
    /// Build a registry with **only** the in-house default over `model`. This is the feature-OFF
    /// registry (and the feature-ON registry before any external profile is registered): exactly the
    /// posture a minimal node has — default-only, no external surface.
    pub fn with_default(model: Arc<dyn ErasedModel>) -> Self {
        let default: Arc<dyn AgentRuntime> = Arc::new(InHouseRuntime::new(model));
        let mut entries = HashMap::new();
        entries.insert(DEFAULT_RUNTIME.to_string(), default);
        Self {
            entries,
            default_id: DEFAULT_RUNTIME.to_string(),
        }
    }

    /// Register an external runtime under its own id. Called by the node's `external-agent`
    /// registration hook (feature-gated) with the `AcpRuntime` the role crate supplies. Registering
    /// is purely additive — a node without the feature never calls this, so the entry is absent and
    /// the OFF build carries none of the external code.
    pub fn register(&mut self, runtime: Arc<dyn AgentRuntime>) {
        self.entries.insert(runtime.id().to_string(), runtime);
    }

    /// The default runtime (always present).
    pub fn default_runtime(&self) -> &Arc<dyn AgentRuntime> {
        self.entries
            .get(&self.default_id)
            .expect("default runtime is always registered")
    }

    /// Resolve a `runtime` argument to a runtime, per the decided rules (see module docs).
    pub fn resolve(&self, runtime: Option<&str>) -> Result<&Arc<dyn AgentRuntime>, AgentError> {
        match runtime {
            // Absent → default.
            None => Ok(self.default_runtime()),
            // Explicitly named → must exist; an unknown named runtime is an error, never a silent
            // downgrade to a different engine.
            Some(id) => self
                .entries
                .get(id)
                .ok_or_else(|| AgentError::BadInput(format!("unknown runtime {id:?}"))),
        }
    }

    /// The configured runtime ids (for the `agent.runtimes` read verb, #5 — TODO). Sorted for a
    /// stable listing; `default` is always among them.
    pub fn ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.entries.keys().cloned().collect();
        ids.sort();
        ids
    }
}
