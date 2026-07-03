//! The host-owned **`AgentRuntime` seam** (external-agent sub-scope #1, runtime-seam) — the twin of
//! [`ModelAccess`](super::model_access), one level up. Where `ModelAccess` abstracts "one model
//! turn", `AgentRuntime` abstracts "**run a bounded loop toward a goal**": drive an agent to a
//! terminal outcome, using only the tools at a given MCP endpoint (the workspace-walled [`Node`]),
//! emitting [`RunEvent`]s, returning the final answer when done or the ceiling is hit.
//!
//! **Why it lives in host, not a role.** Same reason as `ModelAccess`: `lb-host` owns the trait so
//! callers (`agent.invoke`, a job, the UI) compile **identically** whether or not the external path
//! is built. The in-house loop is a blanket-registered default impl ([`in_house`](super::in_house));
//! the external `AcpRuntime` (sub-scope #2) is supplied by the feature-gated `lb-role-external-agent`
//! crate, which depends on host — never the reverse (symmetric layering, rule 1). The difference
//! between a node that can drive an external agent and one that can't is the **cargo feature + config
//! that populates the [`registry`](super::registry)**, never an `if cloud {…}` branch.
//!
//! **Object-safe on purpose.** The registry holds `Box<dyn AgentRuntime>`, so the call site
//! dispatches through the trait object without enumerating runtime kinds (registry, not a `match`).
//! `ModelAccess::turn` returns `impl Future`, so it is *not* object-safe; the in-house default erases
//! it behind [`ErasedModel`] to become a storable trait object without changing `ModelAccess`.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lb_auth::Principal;

use super::error::AgentError;
use super::model_access::{AllowedTool, ModelAccess, Turn};
use crate::boot::Node;

/// Everything a runtime needs to drive one run — assembled by `agent.invoke` and handed to whichever
/// runtime the registry resolved. Deliberately caller-agnostic: it carries the workspace (the hard
/// wall), the goal, the **derived** caller principal (`agent ∩ caller`, from which the runtime
/// re-derives its own session actor), the MCP endpoint (the `Node` — the *only* tool surface a
/// runtime may touch), the allowed tools, the durable job id, and the wall-clock stamp.
///
/// It intentionally does **not** carry a `ModelAccess`: the in-house default binds its model when it
/// is registered (erased behind [`ErasedModel`]); an external runtime reaches its model through the
/// gateway's OpenAI-compatible endpoint (#4), not this trait.
pub struct RunContext<'a> {
    /// The workspace this run is bound to — the isolation wall. A run for `ws=A` can never name,
    /// read, or signal `ws=B` (enforced downstream at the MCP chokepoint, #3, and the job, #5).
    pub ws: &'a str,
    /// The durable session/job id. Idempotent on this id (resume continues the same record).
    pub job_id: &'a str,
    /// The prompt / task.
    pub goal: &'a str,
    /// The caller principal. The runtime derives its session actor as `agent_caps ∩ caller.caps`
    /// (no widening) — the same derivation the in-house loop performs.
    pub caller: &'a Principal,
    /// The agent actor's own capabilities; the effective grant is `agent_caps ∩ caller.caps`.
    pub agent_caps: &'a [String],
    /// The qualified MCP tools the run may propose. For an external agent this is the granted set the
    /// bridge (#2/#3) exposes; for the in-house loop it is the model's allowed-tools list.
    pub tools: &'a [AllowedTool],
    /// An optional **per-run model override** (active-agent-wiring #2). When set, the in-house
    /// [`InHouseRuntime`](super::in_house) drives the run with THIS model — the workspace's active pick,
    /// resolved at run start via `resolve_workspace_model` — instead of the model bound at registration
    /// (the node-level `LB_AGENT_MODEL_*` fallback). `None` keeps the registered model (the fallback
    /// tier). External runtimes ignore it (they reach their model over their own transport, #4).
    pub model_override: Option<Arc<dyn ErasedModel>>,
    /// Wall-clock stamp (test seam: a fixed clock in tests, live at boot).
    pub ts: u64,
}

/// The host-owned runtime seam. **Object-safe** so a heterogeneous registry can hold
/// `Box<dyn AgentRuntime>` and the call site dispatches without a `match` over kinds.
///
/// `run` drives one bounded loop to a terminal outcome and returns the final answer. A tool denial
/// *inside* the run is fed back to the agent, never surfaced as an error (agent-scope deny path);
/// `run` errors only on a gate refusal at the surface or a store failure — identical to the in-house
/// `invoke` contract, so the two impls are interchangeable behind the seam.
pub trait AgentRuntime: Send + Sync {
    /// The runtime's registry id (the `runtime` selector). `"default"` for the in-house loop; a
    /// profile id (`"open-interpreter-default"`, `"vtcode-default"`, …) for an external runtime.
    fn id(&self) -> &str;

    /// Run one session to completion. Boxed future (not `impl Future`) so the trait stays
    /// object-safe. `RunEvent`s are published onto the run's bus subject as motion (the in-house loop
    /// already does this via `publish_run_event`; the external driver forwards its projected events
    /// the same way) — so a watcher observes an external run identically to an in-house one.
    fn run<'a>(
        &'a self,
        node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>>;
}

/// An object-safe erasure of [`ModelAccess`], so the in-house loop — which is generic over
/// `M: ModelAccess` (whose `turn` returns `impl Future`, not object-safe) — can be stored as a
/// `Box<dyn AgentRuntime>` in the registry. The blanket impl below adapts *any* `ModelAccess` into
/// this boxed-future shape; nothing about `ModelAccess` changes.
pub trait ErasedModel: Send + Sync {
    /// One model turn, boxed. Mirrors [`ModelAccess::turn`] exactly, minus the `impl Future`.
    fn turn_boxed<'a>(
        &'a self,
        ws: &'a str,
        messages: &'a [(String, String)],
        tools: &'a [AllowedTool],
        prior: &'a [super::model_access::CallOutcome],
        idempotency_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Turn> + Send + 'a>>;

    /// Mirrors [`ModelAccess::is_configured`] through the erasure — whether this is a real provider
    /// vs. the placeholder. Lets a registry-stored (erased) model tell a non-agent caller (the rules
    /// engine) whether AI is configured, without un-erasing the concrete type.
    fn is_configured(&self) -> bool;
}

impl<M: ModelAccess + Send + Sync> ErasedModel for M {
    fn turn_boxed<'a>(
        &'a self,
        ws: &'a str,
        messages: &'a [(String, String)],
        tools: &'a [AllowedTool],
        prior: &'a [super::model_access::CallOutcome],
        idempotency_key: &'a str,
    ) -> Pin<Box<dyn Future<Output = Turn> + Send + 'a>> {
        Box::pin(self.turn(ws, messages, tools, prior, idempotency_key))
    }

    fn is_configured(&self) -> bool {
        ModelAccess::is_configured(self)
    }
}

/// Adapt an [`ErasedModel`] back to a [`ModelAccess`] so `run_session` (generic over `M: ModelAccess`)
/// can be driven with the erased, registry-stored model. This is the round-trip that lets the in-house
/// loop be *both* a trait object in the registry and a `ModelAccess` consumer, with no second loop.
pub(crate) struct ModelHandle(pub Arc<dyn ErasedModel>);

impl ModelAccess for ModelHandle {
    fn turn(
        &self,
        ws: &str,
        messages: &[(String, String)],
        tools: &[AllowedTool],
        prior: &[super::model_access::CallOutcome],
        idempotency_key: &str,
    ) -> impl Future<Output = Turn> + Send {
        // Clone the model turn's inputs into an owned future so the returned `impl Future` captures
        // no borrow beyond `&self`'s erased model — matching the trait's elided RPITIT shape, which
        // the in-house loop consumes by `.await`ing immediately (no cross-await borrow held).
        let model = self.0.clone();
        let ws = ws.to_string();
        let messages = messages.to_vec();
        let tools = tools.to_vec();
        let prior = prior.to_vec();
        let key = idempotency_key.to_string();
        async move { model.turn_boxed(&ws, &messages, &tools, &prior, &key).await }
    }

    fn is_configured(&self) -> bool {
        self.0.is_configured()
    }
}
