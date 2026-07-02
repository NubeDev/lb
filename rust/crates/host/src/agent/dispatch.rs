//! `invoke_via_runtime` — the runtime-seam #1 entry that resolves a `runtime` id against the
//! [`RuntimeRegistry`] and dispatches through the [`AgentRuntime`] trait object. This is the ONE
//! place `runtime` selection happens; every caller (`agent.invoke`, a job, the UI) reaches a run the
//! same way and gets the same events/answer regardless of which engine served it.
//!
//! The invoke **gate** (`mcp:agent.invoke:call`, workspace-first) fires here — identically for a
//! default-runtime and an external-runtime invoke (the gate is the same; choosing a runtime is an
//! argument, not a new grant). Substrate (skill/doc) loading stays with the in-house `invoke`; an
//! external runtime that wants a persona loads it itself via the grant-gated `load_skill` (#2), so
//! this seam does not bake the in-house substrate step into the external path.

use lb_auth::Principal;

use super::authorize::authorize_invoke;
use super::error::AgentError;
use super::in_house::DEFAULT_RUNTIME;
use super::model_access::AllowedTool;
use super::registry::RuntimeRegistry;
use super::resolve_default::resolve_effective_runtime;
use super::runtime::RunContext;
use super::substrate::{load_substrate_skill, read_substrate_doc};
use crate::boot::Node;

/// The substrate a caller may seed a run with — a granted `skill` and/or a shared `doc`, loaded under
/// the derived principal before the run. Mirrors [`Invocation`](super::invoke::Invocation)'s fields.
#[derive(Default)]
pub struct Substrate<'a> {
    pub skill: Option<&'a str>,
    pub doc: Option<&'a str>,
}

/// Resolve `runtime` against `registry` and run it. `runtime` follows the decided resolution rules,
/// now with the workspace default folded in: **explicit arg → workspace `agent.config.default_runtime`
/// → registry default** (absent-and-unset → default; named-unknown → error; stored-but-unavailable →
/// registry default, fail-open). Returns the run's final answer. The precedence lives in ONE place
/// ([`resolve_effective_runtime`]); both entrypoints (`agent.invoke` via `serve`, the channel `/agent`
/// worker) reach it here, so they resolve identically.
///
/// **Substrate is the in-house loop's mechanism, applied only for the default runtime.** For the
/// default path the granted skill/doc are baked into the goal exactly as [`invoke`](super::invoke)
/// does (the S4 three gates fire under the caller). An external runtime loads its persona itself via
/// the grant-gated `load_skill` (#2, best-effort persona), so the skill/doc are NOT smuggled into its
/// goal here — the external agent's persona/tools are its profile's concern, not this seam's.
#[allow(clippy::too_many_arguments)]
pub async fn invoke_via_runtime(
    node: &Node,
    registry: &RuntimeRegistry,
    runtime: Option<&str>,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    job_id: &str,
    goal: &str,
    substrate: Substrate<'_>,
    tools: &[AllowedTool],
    ts: u64,
) -> Result<String, AgentError> {
    // Gate 1: may this caller invoke the agent at all? (workspace-first, then mcp:agent.invoke:call).
    // Same gate for every runtime — the external path adds no new caller capability.
    authorize_invoke(caller, ws)?;

    // Resolve the EFFECTIVE runtime id (the one seam): explicit arg → workspace stored default →
    // registry default. Runs AFTER the gate (an unauthorized caller is refused before we read any
    // config) and does NOT widen anything — it is pure selection. A stored id the node no longer
    // offers falls back to the default (fail-open); an explicit id is returned verbatim so the
    // registry lookup below still errors on a named-unknown (no silent downgrade). See resolve_default.
    let effective = resolve_effective_runtime(node, registry, ws, runtime).await;

    // Selection is a registry lookup, not a `match` over kinds. An unknown named runtime errors here.
    let runtime = registry.resolve(effective.as_deref())?;

    // Bake substrate into the goal for the DEFAULT (in-house) runtime only — behaviourally identical
    // to `invoke`. The S4 gates (membership/ownership/grant) fire under the caller (see substrate.rs).
    let mut goal = goal.to_string();
    if runtime.id() == DEFAULT_RUNTIME {
        if let Some(skill_id) = substrate.skill {
            let body = load_substrate_skill(&node.store, caller, agent_caps, ws, skill_id).await?;
            goal = format!("{goal}\n\n[skill {skill_id}]\n{body}");
        }
        if let Some(doc_id) = substrate.doc {
            let content = read_substrate_doc(&node.store, caller, agent_caps, ws, doc_id).await?;
            goal = format!("{goal}\n\n[doc {doc_id}]\n{content}");
        }
    }

    let ctx = RunContext {
        ws,
        job_id,
        goal: &goal,
        caller,
        agent_caps,
        tools,
        ts,
    };
    runtime.run(node, ctx).await
}
