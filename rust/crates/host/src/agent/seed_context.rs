//! Seed a run's **live context** — the system-message framing injected once per run/resume, never
//! persisted to the transcript (each segment re-injects cleanly, no rehydrate double-up):
//!
//!   1. the granted-skills **catalog** (Part 5 — name+description, what the model may
//!      `skill.activate`), filtered to the active persona's pinned set when one applies;
//!   2. the **memory index** (agent-memory scope), read under an ON-BEHALF-OF principal — the
//!      CALLER's sub (so `member:{user}` resolves to the human behind the run) with the agent's
//!      intersected caps (the read can never widen); best-effort, a deny/empty is no injection;
//!   3. on resume, the **bodies of previously-activated skills** (`rehydrate` folds the ids, not
//!      the text) — each reloaded under the grant gate, so a skill revoked between segments drops.
//!
//! Extracted from `run.rs` verbatim (FILE-LAYOUT: the loop file stays within budget; this is the
//! "seed the context" phase, one verb).

use std::sync::Arc;

use lb_auth::Principal;

use super::catalog::render_catalog_filtered;
use super::error::AgentError;
use super::memory::memory_index_for_injection;
use super::rehydrate::LoopState;
use crate::assets::load_skill;
use crate::boot::Node;

/// Push the catalog / memory-index / re-activated-skill system messages onto `state.messages`.
/// `agent` is the derived principal (grant-gated reads); `caller` supplies the on-behalf-of sub
/// for the memory read.
pub(super) async fn inject_context(
    node: &Arc<Node>,
    agent: &Principal,
    caller: &Principal,
    agent_caps: &[String],
    ws: &str,
    persona_catalog: Option<&[String]>,
    state: &mut LoopState,
) -> Result<(), AgentError> {
    // The granted-skills catalog, rendered once per run (grant- + ws-gated; an ungranted skill
    // never reaches the text). Persona-filtered when a persona is active — the wall is the grant
    // either way, filtering only removes already-granted entries.
    if let Some(catalog) = render_catalog_filtered(node, agent, ws, persona_catalog).await? {
        state.messages.push(("system".into(), catalog));
    }

    // The memory index, AFTER the catalog, framed as recalled background. On-behalf-of: the
    // caller's sub with the agent's intersected caps — the bare `agent:session` sub would resolve
    // `member:agent:session` and miss the caller's own memory.
    let on_behalf = caller.derive(caller.sub(), agent_caps.to_vec());
    if let Some(index) = memory_index_for_injection(&node.store, &on_behalf, ws).await {
        state.messages.push(("system".into(), index));
    }

    // Resume: re-inject the bodies of skills activated in a prior segment (Part 5 survives Part-0
    // resume). Fresh run → `active_skills` empty → no-op.
    for id in state.active_skills.clone() {
        if let Ok(skill) = load_skill(&node.store, agent, ws, &id, None).await {
            state
                .messages
                .push(("system".into(), format!("[skill {id}]\n{}", skill.body)));
        }
    }
    Ok(())
}
