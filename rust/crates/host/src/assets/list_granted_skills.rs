//! List the workspace's **granted** skills as a catalog — id + title + description + tier ONLY,
//! never the body (agent-run scope Part 5, "grant gates the set, the model picks within it";
//! core-skills scope, "one agent-facing catalog").
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
//! **Two tiers, one catalog (core-skills scope).** A granted `core.*` id resolves its
//! description/latest from the reserved system namespace; a user id from the workspace namespace.
//! Both carry a `tier` so the agent (and any UI) can tell platform-shipped from workspace-authored.
//! A *deprecated* user id is skipped (hidden from list/latest); core ids are never deprecated.

use lb_assets::{is_core, is_deprecated, list_core_skill_versions, list_skill_grants, list_skills};
use lb_auth::Principal;
use lb_caps::Action;
use lb_store::Store;

use super::authorize::authorize_skill;
use super::error::AssetError;

/// The skill tier: platform-shipped (`core.*`, seeded, read-only) or workspace-authored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillTier {
    Core,
    User,
}

impl SkillTier {
    /// The wire string for the catalog row (`"core"` | `"user"`).
    pub fn as_str(self) -> &'static str {
        match self {
            SkillTier::Core => "core",
            SkillTier::User => "user",
        }
    }
}

/// One catalog entry the model may `skill.activate` — the cheap descriptor, never the body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillCatalogEntry {
    /// The stable skill id (doubles as the title at S4 — see module docs).
    pub id: String,
    /// The skill's human-readable title. At S4 this is the `id` (no separate title column).
    pub title: String,
    /// The skill's prose description — what it is for, so the model can choose.
    pub description: String,
    /// The latest published/seeded version of this id (for a UI / pinned-load hint).
    pub latest: String,
    /// Which tier the skill belongs to (core = platform-shipped, user = workspace-authored).
    pub tier: SkillTier,
}

/// List the granted skills of workspace `ws` for `principal` as a catalog (id + title +
/// description + tier). Workspace-walled and capability-gated; only skills with a live grant are
/// returned. A grant with no published version (a dangling grant, or a fully-deprecated user id) is
/// silently skipped (it cannot be activated anyway).
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
        if is_core(&id) {
            // Core tier: the description/latest come from the reserved system namespace.
            if let Some(latest) = list_core_skill_versions(store, &id).await?.pop() {
                catalog.push(SkillCatalogEntry {
                    title: latest.id.clone(),
                    id: latest.id,
                    description: latest.description,
                    latest: latest.version,
                    tier: SkillTier::Core,
                });
            }
            continue;
        }
        // User tier: a deprecated id is hidden from the catalog (list/latest), like `load_skill`.
        if is_deprecated(store, ws, &id).await? {
            continue;
        }
        if let Some(latest) = list_skills(store, ws, &id).await?.pop() {
            catalog.push(SkillCatalogEntry {
                title: latest.id.clone(),
                id: latest.id,
                description: latest.description,
                latest: latest.version,
                tier: SkillTier::User,
            });
        }
    }
    Ok(catalog)
}
