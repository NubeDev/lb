//! `apply.rs` — the single run-assembly filter both runtimes call through `invoke_via_runtime`
//! (persona-model scope, "the whole point"). Given the effective persona, it:
//!   1. **Enforces the runtime restriction** (persona-coding #4): if the persona pins `runtimes` and
//!      the resolved runtime isn't among them, the run fails at start with a named error — before any
//!      model spend (the extension-builder is in-house-only until the external sandbox ships).
//!   2. **Narrows the menu**: `tools ∩ persona.granted_tools` (glob = opaque trailing-`*` prefix
//!      match). The result is the `AllowedTool` list handed to `RunContext` — so the in-house loop's
//!      model menu AND the external bridge's advertised set are both this narrowed list. The wall is
//!      untouched: every dispatch still re-runs `caps::check`; a persona listing a tool the caller
//!      lacks changes nothing (it was never in `reachable_tools`), and a granted tool the persona
//!      omits is simply un-advertised (a model that proposes it anyway hits the unchanged wall).
//!   3. **Builds the identity + pinned-skill goal fold**: the identity leads, then each
//!      `grounding_skills` body loaded via the shipped grant-gated `load_substrate_skill` path
//!      (**fail-closed**: an ungranted pinned skill errors the run here, before any model call). Baked
//!      into the goal, this reaches BOTH runtimes (the goal seeds the in-house rehydrate and is the
//!      external agent's only channel). The catalog is filtered to the pinned set separately (the
//!      caller passes `effective.grounding_skills` to the persona-aware `render_catalog`).
//!
//! **Narrowing, never widening.** Nothing here grants anything. Tool narrowing is a set intersection
//! over an already-authorized menu; skill loading re-runs the S4 grant gate under the derived
//! principal. The persona is *advertisement + grounding + supervision*, never authorization.

use lb_auth::Principal;

use super::model::PolicyPreset;
use super::resolve::EffectivePersona;
use crate::boot::Node;
// `error`, `model_access`, `substrate` are siblings of `personas` under `agent` — reached as
// `super::…` (this file is `agent::personas::apply`, so `super` is `agent::personas`; `super::super`
// is `agent`). Import them via the crate-agent path re-exports would force `pub`, so use the module
// path directly.
use super::super::error::AgentError;
use super::super::model_access::AllowedTool;
use super::super::policy::{Effect, Policy};
use super::super::substrate::load_substrate_skill;

/// Does `tool_id` match a persona `granted_tools` entry? A plain entry matches literally; a
/// trailing-`*` entry matches by prefix (the `*` stripped). One meaning, nothing smarter — no
/// cap-grammar interplay (scope: "glob semantics rot" guard).
pub fn glob_matches(entry: &str, tool_id: &str) -> bool {
    match entry.strip_suffix('*') {
        Some(prefix) => tool_id.starts_with(prefix),
        None => entry == tool_id,
    }
}

/// Narrow `tools` to those matched by any `granted_tools` entry. An **empty** `granted_tools` on the
/// effective persona means a tool-less conversational persona (the scope's decided semantics: `[]` is
/// "no tools", distinct from an *unset* persona which never reaches here) → the empty menu.
pub fn narrow_tools(tools: &[AllowedTool], granted_tools: &[String]) -> Vec<AllowedTool> {
    tools
        .iter()
        .filter(|t| granted_tools.iter().any(|g| glob_matches(g, &t.name)))
        .cloned()
        .collect()
}

/// Enforce the persona's `runtimes` restriction against the resolved runtime id. `Ok(())` when the
/// persona has no restriction or `runtime_id` is allowed; a named `AgentError::Denied`-shaped failure
/// otherwise (surfaced as a run-start error, no model spend).
pub fn check_runtime(effective: &EffectivePersona, runtime_id: &str) -> Result<(), AgentError> {
    if let Some(allowed) = effective.runtimes.as_ref().filter(|r| !r.is_empty()) {
        if !allowed.iter().any(|r| r == runtime_id) {
            return Err(AgentError::PersonaRuntime {
                persona: effective.id.clone(),
                runtime: runtime_id.to_string(),
                allowed: allowed.clone(),
            });
        }
    }
    Ok(())
}

/// Build the identity + pinned-skill-body fold for the goal. Returns the text to append (or `None`
/// when the persona has neither identity nor pins). Skill bodies load via `load_substrate_skill` under
/// the derived principal — **fail-closed**: an ungranted/unreadable pinned skill returns the load
/// error (the run fails at start with the named skill error, before any model call). The identity
/// leads so it frames the pinned bodies that follow.
pub async fn build_identity_fold(
    node: &Node,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    effective: &EffectivePersona,
) -> Result<Option<String>, AgentError> {
    let mut fold = String::new();
    if !effective.identity.is_empty() {
        fold.push_str("[persona ");
        fold.push_str(&effective.id);
        fold.push_str("]\n");
        fold.push_str(&effective.identity);
    }
    for skill_id in &effective.grounding_skills {
        // Fail-closed: an ungranted pinned skill errors the whole run here (the acp-driver decision,
        // kept) — a persona that promises grounding it cannot deliver must not run half-grounded.
        let body = load_substrate_skill(&node.store, caller, agent_caps, ws, skill_id)
            .await
            .map_err(|_| AgentError::PersonaSkill {
                persona: effective.id.clone(),
                skill: skill_id.clone(),
            })?;
        if !fold.is_empty() {
            fold.push_str("\n\n");
        }
        fold.push_str("[skill ");
        fold.push_str(skill_id);
        fold.push_str("]\n");
        fold.push_str(&body);
    }
    if fold.is_empty() {
        Ok(None)
    } else {
        Ok(Some(fold))
    }
}

/// Apply a persona's `policy_preset` as a **FLOOR** over the workspace policy's evaluated effect for a
/// single tool call (persona-coding #4). Given `ws_effect` (what `evaluate(ws_policy, tool)` returned)
/// this returns the *effective* effect after the floor.
///
/// **Why a clamp and not a merged rule list:** the shared evaluator's precedence is Deny > Allow > Ask
/// (an Ask is the *weakest* — "if any rule already Allows, there's nothing to ask"). That is correct
/// for an admin policy, but it means a preset **Ask** appended as a rule can NEVER beat a blanket
/// `*`-Allow — the supervision floor would silently evaporate. So the floor is applied as a clamp:
///
///   - preset **Deny**  → force Deny (a Deny floor is absolute);
///   - preset **Ask**   → raise Allow to Ask (never *lower* a ws Deny; an existing Ask/Deny stays);
///   - **UNLESS** the workspace policy has an **EXPLICIT** (exact-tool, no-glob) rule for the tool —
///     that explicit rule IS the auditable "loosening is the admin's explicit write" the scope
///     requires, so the ws effect stands. A blanket `*` rule is NOT explicit and does not loosen.
///
/// `preset == None` → `ws_effect` unchanged (no floor).
pub fn clamp_to_preset(
    ws_effect: Effect,
    tool: &str,
    ws_policy: &Policy,
    preset: Option<&PolicyPreset>,
) -> Effect {
    let Some(preset) = preset else {
        return ws_effect;
    };
    // An explicit ws decision about THIS tool (exact match, no glob, no arg-qualifier) means the admin
    // has spoken — the floor yields to it (this is how loosening below the preset is done, auditably).
    let explicit = ws_policy
        .rules
        .iter()
        .any(|r| r.tool == tool && r.arg.is_none());
    if explicit {
        return ws_effect;
    }
    if preset.deny.iter().any(|t| t == tool) {
        return Effect::Deny;
    }
    if preset.ask.iter().any(|t| t == tool) {
        // Raise to Ask, but never weaken an existing Deny (Deny is stricter than Ask).
        return match ws_effect {
            Effect::Deny => Effect::Deny,
            _ => Effect::Ask,
        };
    }
    ws_effect
}
