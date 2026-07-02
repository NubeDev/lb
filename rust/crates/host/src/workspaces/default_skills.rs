//! The **default core-skill grant set** applied at workspace creation (core-skills scope: "Workspace
//! creation applies a configurable default grant set … so a fresh workspace's agent is useful out of
//! the box; an admin can revoke any of them like any other grant").
//!
//! Decided defaults (scope): the **read-only** core skills — `core.lb-cli`, `core.query`,
//! `core.store-read`. Anything that drives writes stays opt-in per admin. The set is node config,
//! overridable at the binary via `LB_DEFAULT_CORE_SKILLS` (comma-separated ids); an empty override
//! grants nothing. This is NOT a grant bypass — each is written as an ordinary `grant:skill/{id}`
//! relation the admin can see and revoke; a fresh workspace with no default set simply starts with
//! an empty catalog until its admin grants.
//!
//! Applied only at **workspace creation** (not a boot fan-out over existing workspaces): an existing
//! workspace's skill set changes only when its admin grants. Best-effort — a grant-write failure
//! never fails the directory write (the workspace existing is the contract; the default grants are
//! the convenience). Idempotent (`relate` upserts the edge).

use lb_assets::relate;
use lb_store::Store;

use crate::assets::{GRANT, GRANT_SCOPE};

/// The compiled-in default set (scope decision). The binary may override via `default_ids`.
pub const DEFAULT_CORE_SKILLS: &[&str] = &["core.lb-cli", "core.query", "core.store-read"];

/// Grant each id in `default_ids` to workspace `ws` as a `grant:skill/{id}` relation. Best-effort:
/// a per-id write error is swallowed (the caller — `workspace_create` — treats the default grants as
/// a convenience, not the contract). Idempotent.
pub async fn grant_default_core_skills(store: &Store, ws: &str, default_ids: &[String]) {
    for id in default_ids {
        let _ = relate(store, ws, GRANT, id, GRANT_SCOPE).await;
    }
}

/// The effective default set: the `LB_DEFAULT_CORE_SKILLS` env override (comma-separated; empty ⇒
/// none) if present, else the compiled-in [`DEFAULT_CORE_SKILLS`]. Read at the binary boundary and
/// threaded in — the crate itself takes the resolved list (no env read inside core, testing §3).
pub fn resolve_default_core_skills(override_csv: Option<&str>) -> Vec<String> {
    match override_csv {
        Some(csv) => csv
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect(),
        None => DEFAULT_CORE_SKILLS.iter().map(|s| s.to_string()).collect(),
    }
}
