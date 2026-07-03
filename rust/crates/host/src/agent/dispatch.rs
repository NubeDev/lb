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

use std::sync::Arc;

use lb_auth::Principal;

use super::authorize::authorize_invoke;
use super::catalog::render_catalog;
use super::error::AgentError;
use super::in_house::DEFAULT_RUNTIME;
use super::memory::memory_index_for_injection;
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
    node: &Arc<Node>,
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

    // Resolve the per-run model override for the DEFAULT (in-house) runtime only (active-agent-wiring
    // #2): the workspace's active pick's `model_endpoint` → a live model, memoized per (ws, endpoint).
    // The in-house loop drives THIS model instead of the node-level `LB_AGENT_MODEL_*` fallback it was
    // registered with. An external runtime reaches its model over its own transport (#4), so no override
    // is resolved for it. Resolved AFTER the gate, under the caller (the definition read inherits the
    // wall).
    //
    // Only override with a **configured** model. When the workspace has no pick and no configured node
    // model, `resolve_workspace_model` returns the honest `UnconfiguredModel` placeholder — but forcing
    // THAT as the override would shadow the model the in-house runtime was actually *registered* with
    // (the registry served through this dispatch, which is not always `node.runtimes()` — e.g. a routed
    // `serve_agent` carrying its own default). So an unconfigured resolution yields `None`, and
    // `InHouseRuntime` falls back to its registered model — preserving the ladder: active pick →
    // registered runtime model → (that model's own) unconfigured answer.
    let model_override = if runtime.id() == DEFAULT_RUNTIME {
        let resolved = super::resolve_workspace_model(node, caller, ws).await;
        resolved.is_configured().then_some(resolved)
    } else {
        None
    };

    // Bake the EXPLICITLY-REQUESTED substrate (a `skill`/`doc` the CALLER named on the invoke) into the
    // goal for BOTH runtimes. An explicit `substrate.skill` is a caller directive, not the model's own
    // menu choice: the AI-widget builder invokes with `skill:core.genui-widget` precisely so the run
    // authors OpenUI-Lang against the catalog — it must NOT be left to the agent to (maybe) self-load.
    // An external agent cannot call the loop-internal `skill.activate`, and a general coding agent
    // (Open Interpreter) that only sees the one-line catalog description ignores the contract entirely
    // and answers with prose → an empty-components IR the host rejects ("IR has no components"). So the
    // body must reach its goal directly, exactly as the in-house loop bakes it. The S4 gates
    // (membership/ownership/grant) fire under the caller (see substrate.rs), so this is not a widening:
    // an ungranted/unreadable skill/doc still fails the read here, identically for either runtime.
    let mut goal = goal.to_string();
    if let Some(skill_id) = substrate.skill {
        let body = load_substrate_skill(&node.store, caller, agent_caps, ws, skill_id).await?;
        goal = format!("{goal}\n\n[skill {skill_id}]\n{body}");
    }
    if let Some(doc_id) = substrate.doc {
        let content = read_substrate_doc(&node.store, caller, agent_caps, ws, doc_id).await?;
        goal = format!("{goal}\n\n[doc {doc_id}]\n{content}");
    }

    // The GRANTED-SKILL CATALOG (the model's own menu, name+description only) differs by runtime:
    // the in-house loop injects its own once per run (run.rs — it can `skill.activate` mid-run), so we
    // do NOT inject it here for the default runtime; an external runtime gets it folded into the goal.
    if runtime.id() != DEFAULT_RUNTIME {
        // EXTERNAL runtime catalog injection (core-skills scope: "both runtimes list granted skills
        // … and inject name+description only"). An external agent's only injection channel is the
        // goal (it drives `ctx.goal` verbatim over `exec --json`), and it cannot call the
        // loop-internal `skill.activate`; so we fold the compact catalog into the goal here, under the
        // DERIVED principal (`caller ∩ agent`) — an ungranted/unreadable skill never reaches the text
        // (render_catalog is grant- + ws-gated, empty catalog → no injection). Bodies stay on demand
        // via the granted `load_skill` tool in the profile's `granted_tools`.
        let agent = caller.derive("agent:session", agent_caps.to_vec());
        if let Some(catalog) = render_catalog(node, &agent, ws).await? {
            goal = format!("{goal}\n\n{catalog}");
        }
        // AGENT MEMORY (agent-memory scope): inject the derived memory index AFTER the skill catalog,
        // under an ON-BEHALF-OF principal — the CALLER's sub (so `member:{user}` resolves to the human
        // behind the run) with the agent's intersected caps (never widening). Best-effort — a
        // deny/empty is simply no injection. An external run RECALLS by default; whether it may `set`
        // is its profile's `granted_tools` opt-in (the read/inject is not a write grant).
        let on_behalf = caller.derive(caller.sub(), agent_caps.to_vec());
        if let Some(index) = memory_index_for_injection(&node.store, &on_behalf, ws).await {
            goal = format!("{goal}\n\n{index}");
        }
        // The persona for an external run is a granted skill it loads itself via `load_skill`
        // (dispatch.rs module docs) — the same loader, pinned by the profile, not a second path.
    }

    let ctx = RunContext {
        ws,
        job_id,
        goal: &goal,
        caller,
        agent_caps,
        tools,
        model_override,
        ts,
    };
    runtime.run(node, ctx).await
}
