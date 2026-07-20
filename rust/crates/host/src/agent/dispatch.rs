//! `invoke_via_runtime` — the runtime-seam #1 entry that resolves a `runtime` id against the
//! [`RuntimeRegistry`] and dispatches through the [`AgentRuntime`] trait object. This is the ONE
//! place `runtime` selection happens; every caller (`agent.invoke`, a job, the UI) reaches a run the
//! same way and gets the same events/answer regardless of which engine served it.
//!
//! The invoke **gate** (`mcp:agent.invoke:call`, workspace-first) fires here — identically for a
//! default-runtime and an external-runtime invoke (the gate is the same; choosing a runtime is an
//! argument, not a new grant). An **explicitly-requested** substrate (a `skill`/`doc` the caller
//! named on the invoke) is baked into the goal for BOTH runtimes — it is a caller directive, not the
//! model's own menu choice (the AI-widget builder invokes with `skill:core.genui-widget` so the run
//! authors OpenUI-Lang; an external agent can't `skill.activate` and must not be left to self-load).
//! The model's own **granted-skills catalog** (name+description, the menu it may activate from) still
//! differs by runtime: the in-house loop renders its own; the external path folds a compact catalog
//! into the goal, and the agent pulls a discovered skill's body on demand via `load_skill` (#2).

use std::sync::Arc;

use lb_auth::Principal;

use super::authorize::authorize_invoke;
use super::catalog::render_catalog_filtered;
use super::error::AgentError;
use super::in_house::DEFAULT_RUNTIME;
use super::memory::memory_index_for_injection;
use super::model_access::AllowedTool;
use super::personas::{
    build_identity_fold, check_runtime, narrow_tools, resolve_effective, resolve_persona,
};
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
/// **An explicitly-requested substrate is baked into the goal for BOTH runtimes.** A `skill`/`doc`
/// the caller named on the invoke is a directive: its body is baked into the goal exactly as
/// [`invoke`](super::invoke) does (the S4 three gates fire under the caller), for the in-house and the
/// external path alike — the AI-widget builder's `skill:core.genui-widget` must reach the model even
/// when the active agent is external. The external agent's own *persona* (an unrequested profile
/// skill) is still its profile's concern, loaded via the grant-gated `load_skill` (#2).
#[allow(clippy::too_many_arguments)]
pub async fn invoke_via_runtime(
    node: &Arc<Node>,
    registry: &RuntimeRegistry,
    runtime: Option<&str>,
    persona: Option<&str>,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    job_id: &str,
    goal: &str,
    substrate: Substrate<'_>,
    context: Option<&serde_json::Value>,
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

    // PERSONA RESOLUTION (agent-personas scope #1) — the run's *focus*, orthogonal to the runtime.
    // Precedence: explicit invoke `persona` → workspace `agent.config.active_persona` → none. Resolved
    // AFTER the gate, under the caller (`agent_persona_get` re-runs its OWN member gate; an explicit
    // unknown id is a named error, a dangling active id warns + runs un-narrowed). The `extends`
    // closure is unioned into an `EffectivePersona`. `None` → the run is un-narrowed (byte-identical to
    // pre-persona behavior below, since `effective_persona` stays `None`).
    let effective_persona = match resolve_persona(node, caller, ws, persona)
        .await
        .map_err(tool_to_agent)?
    {
        Some(p) => Some(
            resolve_effective(node, caller, ws, &p)
                .await
                .map_err(tool_to_agent)?,
        ),
        None => None,
    };

    // RUNTIME RESTRICTION (persona-coding #4): a persona may pin the runtimes it runs under (the
    // extension-builder is in-house-only until the external sandbox ships). Enforced at run start,
    // before any model spend, with a named error — data on the record, never an `if` in core.
    if let Some(ep) = &effective_persona {
        check_runtime(ep, runtime.id())?;
    }

    // MENU NARROWING: the persona's `granted_tools` filter the reachable menu → `persona ∩ reachable`.
    // The wall is UNTOUCHED — this only trims the *advertised* set (the in-house model's proposable
    // tools AND the external bridge's advertised set are both this narrowed list). A persona naming a
    // tool the caller lacks changes nothing (it was never reachable); a granted tool the persona omits
    // is un-advertised but a model that proposes it anyway still hits `caps::check`. `None` persona →
    // the full reachable menu, unchanged.
    let narrowed_tools: Option<Vec<AllowedTool>> = effective_persona
        .as_ref()
        .map(|ep| narrow_tools(tools, &ep.granted_tools));
    let tools: &[AllowedTool] = narrowed_tools.as_deref().unwrap_or(tools);

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

    // PERSONA IDENTITY + PINNED-SKILL FOLD (agent-personas scope #1): the persona's identity leads,
    // then each pinned `grounding_skills` body, baked into the goal for BOTH runtimes (the goal seeds
    // the in-house rehydrate AND is the external agent's only channel — one source, both doors). This
    // is FAIL-CLOSED: an ungranted pinned skill errors the run here with the named `PersonaSkill`
    // error, before any model spend (the acp-driver decision, kept). Placed before the caller's
    // explicit substrate so the identity frames everything that follows.
    if let Some(ep) = &effective_persona {
        if let Some(fold) = build_identity_fold(node, caller, agent_caps, ws, ep).await? {
            goal = format!("{fold}\n\n{goal}");
        }
    }

    if let Some(skill_id) = substrate.skill {
        let body = load_substrate_skill(&node.store, caller, agent_caps, ws, skill_id).await?;
        goal = format!("{goal}\n\n[skill {skill_id}]\n{body}");
    }
    if let Some(doc_id) = substrate.doc {
        let content = read_substrate_doc(&node.store, caller, agent_caps, ws, doc_id).await?;
        goal = format!("{goal}\n\n[doc {doc_id}]\n{content}");
    }

    // Fence the optional client-reported PAGE CONTEXT into the goal (agent-dock scope). This is the ONE
    // seam both front doors reach, so the channel worker and the invoke route fence identically. Absent
    // → `goal` unchanged (byte-identical to today); an oversize object is REJECTED here (a `BadInput`
    // the door maps to a reject). Context is untrusted (the fence says so) and never widens the run —
    // the wall stays the caller's captured caps.
    goal = crate::agent::fence_into_goal(&goal, context)?;

    // The GRANTED-SKILL CATALOG (the model's own menu, name+description only) differs by runtime:
    // the in-house loop injects its own once per run (run.rs — it can `skill.activate` mid-run), so we
    // do NOT inject it here for the default runtime; an external runtime gets it folded into the goal.
    if runtime.id() != DEFAULT_RUNTIME {
        // EXTERNAL runtime catalog injection (core-skills scope: "both runtimes list granted skills
        // … and inject name+description only"). An external agent's only injection channel is the
        // goal (it drives `ctx.goal` verbatim over `exec --json`), and it cannot call the
        // loop-internal `skill.activate`; so we fold the compact catalog into the goal here, under the
        // DERIVED principal (`caller ∩ agent`) — an ungranted/unreadable skill never reaches the text
        // (render_catalog is grant- + ws-gated, empty catalog → no injection). Bodies for a skill the
        // agent DISCOVERS in this catalog stay on demand via the granted `load_skill` tool in the
        // profile's `granted_tools`; an EXPLICITLY-requested skill's body is already baked above.
        let agent = caller.derive("agent:session", agent_caps.to_vec());
        // Filter the advertised catalog to the persona's pinned skills when a persona is active (the
        // model sees the persona's focus, not the whole granted set). `None` persona → the full granted
        // catalog, unchanged. The grant stays the wall — filtering only removes already-granted entries.
        let pinned = effective_persona
            .as_ref()
            .map(|ep| ep.grounding_skills.as_slice());
        if let Some(catalog) = render_catalog_filtered(node, &agent, ws, pinned).await? {
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

    // The in-house loop filters its advertised catalog to the persona's pinned skills (the external
    // runtime already folded its filtered catalog into the goal above and ignores this). Borrows from
    // `effective_persona`, which outlives `ctx`.
    let persona_catalog = effective_persona
        .as_ref()
        .map(|ep| ep.grounding_skills.as_slice());
    // The persona's supervision floor (persona-coding #4) — the in-house loop folds it into the ws
    // policy as an Ask/Deny floor over node-mutating tools. Borrows from `effective_persona`.
    let persona_preset = effective_persona
        .as_ref()
        .and_then(|ep| ep.policy_preset.as_ref());

    let ctx = RunContext {
        ws,
        job_id,
        goal: &goal,
        caller,
        agent_caps,
        tools,
        model_override,
        persona_catalog,
        persona_preset,
        ts,
    };
    runtime.run(node, ctx).await
}

/// Map a persona-resolution [`ToolError`](lb_mcp::ToolError) onto the run's [`AgentError`]. A persona
/// deny stays opaque (`Denied`); a named `BadInput`/`NotFound` (an explicit-but-unknown persona id, a
/// cross-ws access) carries its message through so the caller learns *why* the chosen persona did not
/// apply — an explicit ask must not silently degrade.
fn tool_to_agent(e: lb_mcp::ToolError) -> AgentError {
    match e {
        lb_mcp::ToolError::Denied => AgentError::Denied,
        lb_mcp::ToolError::NotFound => AgentError::NotFound,
        lb_mcp::ToolError::BadInput(m) => AgentError::BadInput(m),
        lb_mcp::ToolError::Extension(m) => AgentError::BadInput(m),
        // A routing failure while resolving a persona (routed-node-dispatch #81) carries its
        // message through as `BadInput`, matching how `Extension` is treated here: an explicit
        // persona ask that could not be honoured must say why rather than silently degrading.
        e @ (lb_mcp::ToolError::Ambiguous { .. }
        | lb_mcp::ToolError::NodeUnreachable { .. }
        | lb_mcp::ToolError::NodeTooOld { .. }) => AgentError::BadInput(e.to_string()),
    }
}
