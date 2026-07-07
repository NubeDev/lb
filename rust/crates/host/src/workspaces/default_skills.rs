//! The **default core-skill grant set** applied at workspace creation (core-skills scope: "Workspace
//! creation applies a configurable default grant set … so a fresh workspace's agent is useful out of
//! the box; an admin can revoke any of them like any other grant").
//!
//! Decided defaults: the **grounding-skill set the built-in persona catalog pins**. A skill is a
//! *doc the agent reads*, not a capability — granting one advertises knowledge, never a write; every
//! tool a grounded agent then proposes still passes the unchanged wall (`persona ∩ agent ∩ caller`).
//! So the default set is deliberately generous on *grounding* while staying strict on *tools*: an
//! admin still grants each capability separately. Without this, every built-in persona except
//! data-analyst fails **fail-closed** at run start (it pins `core.dashboard-mcp`, `core.panels`,
//! `core.channels-inbox-outbox`, … — none of which were in the old 3-skill read-only default), which
//! made the shipped catalog unusable out of the box.
//!
//! The set is node config, overridable at the binary via `LB_DEFAULT_CORE_SKILLS` (comma-separated
//! ids; an empty override grants nothing — a workspace that wants the strict old posture sets it).
//! This is NOT a grant bypass — each is written as an ordinary `grant:skill/{id}` relation the admin
//! can see and revoke; a fresh workspace with an empty override starts with no grounding until its
//! admin grants. Keep this list in sync with the `grounding_skills` in `personas/personas.toml` — a
//! persona pinning a skill absent here fails fail-closed until an admin grants it (that is the
//! contract, but the built-ins should work on a fresh workspace).
//!
//! Applied only at **workspace creation** (not a boot fan-out over existing workspaces): an existing
//! workspace's skill set changes only when its admin grants. Best-effort — a grant-write failure
//! never fails the directory write (the workspace existing is the contract; the default grants are
//! the convenience). Idempotent (`relate` upserts the edge).

use lb_assets::relate;
use lb_store::Store;

use crate::assets::{GRANT, GRANT_SCOPE};

/// The compiled-in default set: every grounding skill the built-in persona catalog
/// (`personas/personas.toml`) pins, so a fresh workspace's built-in personas all start (a pinned
/// skill is fail-closed at run assembly). Grounding = docs the agent reads; the capability wall still
/// gates every tool, so this is generous-on-knowledge, strict-on-tools. Keep in sync with the
/// personas' `grounding_skills`. The binary may override via `LB_DEFAULT_CORE_SKILLS` (empty ⇒ the
/// strict "grant nothing" posture).
pub const DEFAULT_CORE_SKILLS: &[&str] = &[
    // data-analyst
    "core.datasources",
    "core.query",
    "core.store-read",
    "core.ingest-series",
    "core.channel-widgets",
    // flow-author
    "core.flows-mcp",
    // widget-builder
    "core.dashboard-mcp",
    "core.genui-widget",
    "core.panels",
    "core.dashboard-widgets",
    "core.render-widgets",
    // rules-author
    "core.rules",
    // workspace-admin
    "core.nav",
    "core.auth-caps",
    "core.prefs",
    // channels-operator
    "core.channels-inbox-outbox",
    // system-manager
    "core.lb-cli",
    "core.mcp",
    "core.agent",
    // insights-analyst
    "core.insights",
    // extension-builder
    "core.extension-authoring",
    "core.extensions",
    "core.e2e-backend",
];

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
