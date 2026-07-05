//! `resolve_persona` — the ONE shared answer to "which persona is active" for a run (persona-model
//! scope). The definition twin of [`resolve_active_definition`](crate::agent::resolve_active_definition),
//! one concept over: a persona is orthogonal to a definition (persona = *focus*, definition =
//! *(runtime, model)*), so this is its own resolver, not a field on that one.
//!
//! **Precedence (decided):**
//!   1. an **explicit** invoke `persona` id → that persona (`agent_persona_get`, which re-runs its OWN
//!      member gate + built-in/custom namespace split — the wall is inherited, never widened). An
//!      explicit-but-unknown id is a **named error** (an explicit ask must not silently degrade).
//!   2. else the workspace's **`agent.config.active_persona`** id, if set → `agent_persona_get(id)`. A
//!      dangling active id (persona since deleted) → **`warn!` + no persona** (registry-default
//!      behavior, never an errored run — the `resolve_effective_runtime` posture).
//!   3. else **no persona** (the run is un-narrowed, exactly as today).
//!
//! The resolved persona's `extends` closure is unioned in [`resolve_effective`] so a caller gets the
//! full tool/skill surface without walking parents itself.

use lb_auth::Principal;
use lb_mcp::ToolError;

use super::model::{is_builtin, Persona, PolicyPreset};
use super::store::{get_persona, PERSONA_NS};
use crate::agent::get_agent_config;
use crate::boot::Node;

/// Read a persona by id at RUN ASSEMBLY — a namespace-walled RAW store read, deliberately NOT gated on
/// `mcp:agent.persona.get:call`. A persona read at run assembly can only ever *narrow* the run (it
/// removes tools + pins skills, never adds a capability), so requiring the invoking member to
/// personally hold the picker's read cap would be a gate that guards nothing while breaking the common
/// case (a member whose workspace picked a persona must have it apply). The workspace wall still holds:
/// a `builtin.*` id resolves from the reserved namespace (readable everywhere, the built-in union), any
/// other id ONLY from `ws` (a ws-B run can never read a ws-A custom persona). The CRUD *verbs*
/// (`get.rs` etc.) keep their cap gate for the Settings surface — this is the run-assembly seam only.
async fn read_persona_for_assembly(
    node: &Node,
    ws: &str,
    id: &str,
) -> Result<Option<Persona>, ToolError> {
    let (ns, builtin) = if is_builtin(id) {
        (PERSONA_NS, true)
    } else {
        (ws, false)
    };
    get_persona(&node.store, ns, id, builtin)
        .await
        .map_err(|_| ToolError::Denied)
}

/// The materialized effect of an active persona on a run — the union of its own + inherited lists, an
/// identity string (child-wins), the pinned skill-id set, and the optional supervision floor +
/// runtime restriction. Produced by [`resolve_effective`]; consumed by `apply.rs`.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EffectivePersona {
    /// The id of the persona that resolved (for logs / the effective-tools view).
    pub id: String,
    /// The identity prompt (child wins over parents).
    pub identity: String,
    /// The unioned tool allow-list (ids + trailing-`*` globs) — opaque data (rule 10).
    pub granted_tools: Vec<String>,
    /// The unioned pinned-skill ids (deduped, child order first).
    pub grounding_skills: Vec<String>,
    /// The supervision floor, if any (persona-coding #4).
    pub policy_preset: Option<PolicyPreset>,
    /// The runtime restriction, if any (persona-coding #4) — empty/None means no restriction.
    pub runtimes: Option<Vec<String>>,
}

/// Resolve the active persona for `ws` under `caller`: the explicit `persona` id, else the workspace's
/// `active_persona` pick, else none. Returns `Ok(None)` when no persona applies (the run is
/// un-narrowed). An explicit-but-unknown id is a named `BadInput`; a dangling *active* id warns and
/// resolves to `None`.
pub async fn resolve_persona(
    node: &Node,
    // The caller identity is not consulted for the persona READ (a run-assembly persona read is
    // narrowing-only and namespace-walled, not cap-gated — see `read_persona_for_assembly`). Kept in
    // the signature so the seam matches its `resolve_active_definition` sibling and a future
    // caller-aware policy has a place to land without a signature churn.
    _caller: &Principal,
    ws: &str,
    persona: Option<&str>,
) -> Result<Option<Persona>, ToolError> {
    // (1) Explicit invoke override → that persona. An unknown explicit id is a named `NotFound` (an
    // explicit ask must not silently degrade to un-narrowed). Namespace-walled raw read (see
    // `read_persona_for_assembly` — narrowing-only, so not the picker cap gate).
    if let Some(id) = persona.filter(|s| !s.is_empty()) {
        return read_persona_for_assembly(node, ws, id)
            .await?
            .map(Some)
            .ok_or(ToolError::NotFound);
    }

    // Read the workspace pick (best-effort: a store read error is treated as "unset", never a panic).
    let cfg = get_agent_config(&node.store, ws)
        .await
        .map_err(|_| ToolError::Denied)?;

    // (2) The `active_persona` pick, if set and it still resolves. A dangling id (persona since
    // deleted) → warn + no persona, NOT an error (the resolve-at-read posture; the run just isn't
    // narrowed).
    if let Some(active) = cfg
        .as_ref()
        .and_then(|c| c.active_persona.as_deref())
        .filter(|s| !s.is_empty())
    {
        match read_persona_for_assembly(node, ws, active).await {
            Ok(Some(p)) => return Ok(Some(p)),
            Ok(None) | Err(_) => {
                tracing::warn!(
                    "run assembly: active_persona {active:?} did not resolve in ws {ws:?}; running un-narrowed"
                );
                return Ok(None);
            }
        }
    }

    // (3) No persona.
    Ok(None)
}

/// Materialize `persona` into its [`EffectivePersona`] by unioning its `extends` closure. Parents are
/// read (raw, namespace-walled) from the workspace namespace, then the reserved built-in namespace (a custom
/// persona may extend a built-in). Resolution is bounded — the write-time cycle/depth check
/// (`validate.rs`) guarantees termination — but this read walk is defensively depth-capped too. The
/// child's identity wins; tool/skill lists union (child entries first, deduped, order-stable).
pub async fn resolve_effective(
    node: &Node,
    // Not consulted (parent reads are namespace-walled raw reads, narrowing-only). Kept for seam
    // symmetry, as in `resolve_persona`.
    _caller: &Principal,
    ws: &str,
    persona: &Persona,
) -> Result<EffectivePersona, ToolError> {
    let mut granted_tools: Vec<String> = Vec::new();
    let mut grounding_skills: Vec<String> = Vec::new();
    // Child first so its entries lead; parents append what the child doesn't already list.
    push_unique(&mut granted_tools, &persona.granted_tools);
    push_unique(&mut grounding_skills, &persona.grounding_skills);

    // BFS the parents, newest-child-first. A defensive depth cap mirrors the write-time bound so a
    // hand-seeded/imported record that slipped past validation still can't loop forever.
    let mut queue: Vec<(String, usize)> = persona.extends.iter().map(|p| (p.clone(), 1)).collect();
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    visited.insert(persona.id.clone());

    while let Some((id, depth)) = queue.pop() {
        if depth > super::validate::MAX_EXTENDS_DEPTH || !visited.insert(id.clone()) {
            continue;
        }
        let parent = if is_builtin(&id) {
            get_persona(&node.store, PERSONA_NS, &id, true).await
        } else {
            get_persona(&node.store, ws, &id, false).await
        }
        .map_err(|_| ToolError::Denied)?;
        if let Some(parent) = parent {
            push_unique(&mut granted_tools, &parent.granted_tools);
            push_unique(&mut grounding_skills, &parent.grounding_skills);
            for gp in parent.extends {
                queue.push((gp, depth + 1));
            }
        } else {
            tracing::warn!("persona {:?} extends unresolved parent {id:?}", persona.id);
        }
    }

    Ok(EffectivePersona {
        id: persona.id.clone(),
        identity: persona.identity.clone(),
        granted_tools,
        grounding_skills,
        policy_preset: persona.policy_preset.clone(),
        runtimes: persona.runtimes.clone(),
    })
}

/// Append each of `src` to `dst` preserving order, skipping duplicates (a set that keeps insertion
/// order — child entries stay ahead of the parents that also list them).
fn push_unique(dst: &mut Vec<String>, src: &[String]) {
    for s in src {
        if !dst.iter().any(|d| d == s) {
            dst.push(s.clone());
        }
    }
}
