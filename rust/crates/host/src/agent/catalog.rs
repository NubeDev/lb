//! Render the workspace's granted-skills catalog into the run's context (agent-run scope Part 5,
//! "Skills: grant gates the set, the model picks within it"). The loop injects this ONCE per run so
//! the model knows what it may `skill.activate` — title + description only, never the body (the body
//! is pulled on demand by activation).
//!
//! RESOLVED DECISION (scope, "Skill-catalog injection → render once per run, cache, re-inject only
//! on change"): the catalog is computed at run start and injected once, NOT re-rendered every turn
//! (it would pay the catalog token cost on each inference). The granted set changing mid-run is rare;
//! a re-inject-on-change hook is a documented follow-up (see the run.rs call site) — v1 does the
//! once-per-run render + inject, which is the load-bearing half of the decision.
//!
//! Why a separate module and not inline in run.rs: rule 8 (one responsibility per file, run.rs is
//! near its ceiling) — the *rendering* (catalog → a context message) is its own concern, pure and
//! unit-testable, distinct from the loop that consumes it. It reads the granted set via the S4 host
//! verb (`list_granted_skills`, grant- and workspace-gated), so the wall holds: an ungranted skill
//! never reaches the rendered text.

use lb_auth::Principal;

use super::error::AgentError;
use crate::assets::{list_granted_skills, AssetError, SkillCatalogEntry};
use crate::boot::Node;

/// The system-prompt-style header introducing the catalog. One place owns it so a fresh render and
/// any future re-render produce identical framing.
const CATALOG_HEADER: &str = "Skills you may activate with the skill.activate tool \
(call skill.activate with {\"id\": \"<id>\"} to load one's full instructions):";

/// Load + render the granted-skills catalog for `ws` under the on-behalf-of `actor` principal. The
/// read is grant- and workspace-gated (`list_granted_skills`); the result is the context message
/// text, or `None` when the workspace has granted no skills (nothing to inject — do not pay the
/// header's tokens for an empty catalog).
pub async fn render_catalog(
    node: &Node,
    actor: &Principal,
    ws: &str,
) -> Result<Option<String>, AgentError> {
    // A `Denied`/`NotFound` here means the agent lacks the skill-read capability (or there is
    // nothing to read) — that is simply an EMPTY catalog for this run, NOT a run failure. Injecting
    // the catalog is best-effort context, never a gate: a run that cannot see any skill just gets no
    // catalog (and any `skill.activate` it then proposes is denied at activation time anyway). Only a
    // genuine store error propagates.
    match list_granted_skills(&node.store, actor, ws).await {
        Ok(entries) => Ok(format_catalog(&entries)),
        Err(AssetError::Denied) | Err(AssetError::NotFound) => Ok(None),
        Err(AssetError::Store(s)) => Err(AgentError::Store(s)),
    }
}

/// Format a catalog into the single context message the loop injects, or `None` for an empty
/// catalog. Pure (no store/clock) so the rendering is unit-testable independent of the loop.
pub fn format_catalog(entries: &[SkillCatalogEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let mut out = String::from(CATALOG_HEADER);
    for e in entries {
        // One line per skill: `- <id>: <description>`. Title == id at S4 (see list_granted_skills).
        out.push_str("\n- ");
        out.push_str(&e.id);
        out.push_str(": ");
        out.push_str(&e.description);
    }
    Some(out)
}
