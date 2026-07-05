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
use super::super::policy::{Effect, Policy, Rule};
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

/// Fold a persona's `policy_preset` into the workspace `Policy` as a **FLOOR** (persona-coding #4). The
/// preset lists node-mutating tools the persona wants gated (Ask) or refused (Deny); the workspace
/// policy is the admin's own rule list. The floor rule:
///
///   For each preset tool, the preset rule applies **unless the workspace policy already has an
///   EXPLICIT rule for that exact tool** — an explicit ws rule IS the "loosening is the explicit admin
///   write" the scope requires. So: seed Ask on `ext.publish` holds by default; an admin who wants it
///   Allow must write an explicit `{tool:"ext.publish", effect:allow}` (a deliberate, auditable act),
///   not merely have a broad `*`-Allow (which would silently loosen the floor and defeat supervision).
///
/// Precedence within the merged list is the evaluator's own (Deny > Allow > Ask), unchanged. The preset
/// rules are appended AFTER the ws rules; because we only append a preset rule when NO explicit ws rule
/// names that tool, the preset can't fight an explicit admin decision, and a broad ws `*` rule does not
/// count as "explicit for this tool" (so the floor survives a blanket Allow — the load-bearing point).
///
/// `None` preset → the ws policy unchanged.
pub fn apply_policy_preset(ws_policy: Policy, preset: Option<&PolicyPreset>) -> Policy {
    let Some(preset) = preset else {
        return ws_policy;
    };
    let mut rules = ws_policy.rules;
    // A tool is "explicitly named" by the ws policy iff a rule's `tool` equals it verbatim (a `*`-glob
    // is NOT explicit — a blanket Allow must not silently loosen a preset floor).
    let explicit = |tool: &str, rules: &[Rule]| rules.iter().any(|r| r.tool == tool && r.arg.is_none());
    for tool in &preset.deny {
        if !explicit(tool, &rules) {
            rules.push(Rule {
                tool: tool.clone(),
                arg: None,
                effect: Effect::Deny,
            });
        }
    }
    for tool in &preset.ask {
        if !explicit(tool, &rules) {
            rules.push(Rule {
                tool: tool.clone(),
                arg: None,
                effect: Effect::Ask,
            });
        }
    }
    Policy { rules }
}
