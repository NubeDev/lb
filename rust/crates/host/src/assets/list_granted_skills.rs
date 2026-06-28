//! List the workspace's **granted** skills as a catalog — id + title + description ONLY, never
//! the body (agent-run scope Part 5, "grant gates the set, the model picks within it").
//!
//! This is the inverse of [`load_skill`](super::load_skill::load_skill): `load_skill` pulls one
//! granted skill's *full body* on demand; this lists the *catalog* the model chooses from — the
//! cheap "what may I activate" surface injected into the run's context once per run. The grant is
//! still the wall: a skill the workspace did not grant never appears here (so it can never be named
//! to `skill.activate`), and a ws-B caller sees only ws-B's grants (workspace-namespaced list).
//!
//! Gates, in order, exactly like every other asset read:
//!   1. workspace isolation + 2. capability — `authorize_skill(..., Read)` on the `skill/*` surface
//!      (the same `store:skill/*:read` cap `load_skill` requires), checked BEFORE any fetch;
//!   3. grant — only skills with a live `grant` edge are listed (the catalog *is* the grant set).
//!
//! Why title+description only (no body): the catalog pays a per-run token cost (it is injected into
//! context), so it must stay small; the body is loaded on demand by `skill.activate`. Leaking the
//! body here would both bloat the catalog and defeat the "activate on demand" pattern. The `Skill`
//! model has no separate `title` field at S4, so the skill `id` doubles as the title (a stable,
//! human-meaningful name) — the description carries the prose. Rejected: adding a `title` column to
//! the S4 `Skill` (out of scope — Part 5 is purely additive over S4, no skill-store change).

use lb_assets::{list_skill_grants, list_skills};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;

/// One catalog entry the model may `skill.activate` — the cheap descriptor, never the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    /// The stable skill id (doubles as the title at S4 — see module docs).
    pub id: String,
    /// The skill's human-readable title. At S4 this is the `id` (no separate title column).
    pub title: String,
    /// The skill's prose description — what it is for, so the model can choose.
    pub description: String,
}

/// List the granted skills of workspace `ws` for `principal` as a catalog (id + title +
/// description). Workspace-walled and capability-gated; only skills with a live grant are
/// returned. A skill grant with no published version is silently skipped (a dangling grant is not
/// a catalog entry — it cannot be activated anyway).
pub async fn list_granted_skills(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<SkillCatalogEntry>, AssetError> {
    // Gates 1 + 2: workspace, then the skill read capability — before any fetch. Same surface as
    // `load_skill`, on the `*` wildcard (the catalog spans every granted skill).
    authorize_skill(principal, ws, "*", Action::Read)?;

    // Gate 3: the catalog IS the grant set — only granted skill ids, never another workspace's.
    let ids = list_skill_grants(store, ws).await?;

    let mut catalog = Vec::with_capacity(ids.len());
    for id in ids {
        // Latest published version carries the current description; the body is intentionally
        // dropped (catalog stays cheap). A dangling grant (no published version) is skipped.
        if let Some(latest) = list_skills(store, ws, &id).await?.pop() {
            catalog.push(SkillCatalogEntry {
                title: latest.id.clone(),
                id: latest.id,
                description: latest.description,
            });
        }
    }
    Ok(catalog)
}
