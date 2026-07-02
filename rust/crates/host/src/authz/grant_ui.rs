//! `grant_ui_scope_to_admin` — on install, make an extension's PAGE/WIDGET tool surface reachable by
//! workspace admins through the durable grant store, so no extension ever edits the login path
//! (authz-grants scope: "granting an extension's tool to a user/team is an ordinary grant"; the
//! session token is a cached projection of `resolve_caps`).
//!
//! What it grants: each tool named in the manifest's `[ui].scope` (and every `[[widget]].scope`),
//! **intersected with what was actually `granted`** at install (`requested ∩ admin_approved`), as an
//! `mcp:<tool>:call` capability to the built-in `role:workspace-admin`. On the next login,
//! `resolve_caps` expands that role for any admin, so the page's `bridge.call`s pass the host gate.
//!
//! Why the role (not the installing user): the grant outlives the installer and applies to every
//! admin uniformly; it is revocable from the admin console like any grant. Why only the UI scope (not
//! the full `granted` set): the UI scope is the precise user-facing surface the page drives — the
//! ext's internal caps (store/net/callback verbs) stay with the sidecar's own token, never a user's.
//!
//! Best-effort: a store hiccup here never fails the install (the durable `Install` record is already
//! written; a missing grant just means the page 403s until re-granted — visible, not silent
//! corruption). System effect via the raw `grant_assign` (no gated caller — same reasoning as the
//! first-member bootstrap in `membership/login_resolve.rs`).

use lb_authz::{grant_assign, Subject};
use lb_ext_loader::Manifest;
use lb_store::Store;

/// The built-in role a workspace's admins hold (mirrors `membership::WORKSPACE_ADMIN_ROLE_CAP` without
/// its `role:` prefix — this is the role NAME, the grant subject).
const WORKSPACE_ADMIN_ROLE: &str = "workspace-admin";

/// Grant the manifest's `[ui]`/`[[widget]]` scope tools (∩ `granted`) to `role:workspace-admin` in
/// `ws`. Idempotent (re-install re-upserts the same grant rows). Best-effort — logs, never fails.
pub async fn grant_ui_scope_to_admin(
    store: &Store,
    ws: &str,
    manifest: &Manifest,
    granted: &[String],
) {
    let admin = Subject::Role(WORKSPACE_ADMIN_ROLE.to_string());
    for tool in ui_scope_tools(manifest) {
        let cap = format!("mcp:{tool}:call");
        // Only grant what the install actually granted — never widen beyond `requested ∩ approved`.
        if !granted.iter().any(|g| g == &cap) {
            continue;
        }
        if let Err(e) = grant_assign(store, ws, &admin, &cap).await {
            eprintln!(
                "grant_ui_scope_to_admin: {} → {cap} skipped ({e})",
                manifest.id
            );
        }
    }
}

/// The de-duplicated set of tool names the manifest's page + widgets declare in their `scope`.
fn ui_scope_tools(manifest: &Manifest) -> Vec<String> {
    let mut tools: Vec<String> = Vec::new();
    if let Some(ui) = manifest.ui.as_ref() {
        tools.extend(ui.scope.iter().cloned());
    }
    for w in &manifest.widgets {
        tools.extend(w.scope.iter().cloned());
    }
    tools.sort();
    tools.dedup();
    tools
}
