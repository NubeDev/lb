//! `grant_role_tiers` — on install, register an extension's DECLARED role-tier caps into the
//! workspace's role records (ext-role-tiers scope). This is what lets `builtin_roles.rs` stay free
//! of extension names: an extension says which tier may call each of its tools (`[[tools]] role =
//! "viewer" | "member" | "admin"`), and the install writes `mcp:<tool>:call` grants to the matching
//! built-in role subjects through the SAME durable grant store an admin edits. `resolve_caps`
//! already unions stored role grants on top of the live built-in bundles, so the caps reach every
//! holder's next minted token — no login-path edit, no core edit, no per-extension code anywhere.
//!
//! Tier → roles mapping (a lower tier is contained in every higher one, mirroring how the built-in
//! bundles nest: member ⊇ viewer, admin ⊇ member):
//!
//!   viewer → role:viewer, role:member, role:workspace-admin
//!   member → role:member, role:workspace-admin
//!   admin  → role:workspace-admin
//!
//! **The cap is the QUALIFIED name.** A `[[tools]]` name is bare by construction and is callable — and
//! cap-gated — as `<ext>.<name>` ([`crate::ui_decl::qualify_tool`], the one place that rule lives).
//! The install's `granted` set is spelled the same way (`mcp:<ext>.<name>:call`), so the tier grant must
//! be too: built from the bare name it matched nothing in `granted`, and every declared tier was
//! dropped without a word.
//!
//! An undeclared tool gets NOTHING here (fail-closed — reachable only through whatever the UI-scope
//! admin grant or an explicit console grant supplies). Everything is intersected with `granted`
//! (`requested ∩ admin_approved`) — a tier declaration can never widen past what the install
//! actually approved. Idempotent (re-install re-upserts the same grant rows), best-effort (a store
//! hiccup logs and never fails the install — the same posture as `grant_ui.rs`, whose docstring
//! carries the full reasoning), and revocable from the admin console like any grant.

use lb_authz::{grant_assign, Subject};
use lb_ext_loader::Manifest;
use lb_store::Store;

/// The built-in role names (the grant subjects). Mirrors `builtin_roles::ROLE_*` values.
const ROLE_VIEWER: &str = "viewer";
const ROLE_MEMBER: &str = "member";
const ROLE_WORKSPACE_ADMIN: &str = "workspace-admin";

/// The roles a declared tier reaches — the containment chain above, as data.
fn roles_for(tier: &str) -> &'static [&'static str] {
    match tier {
        "viewer" => &[ROLE_VIEWER, ROLE_MEMBER, ROLE_WORKSPACE_ADMIN],
        "member" => &[ROLE_MEMBER, ROLE_WORKSPACE_ADMIN],
        "admin" => &[ROLE_WORKSPACE_ADMIN],
        // The loader rejects unknown tiers at parse; this arm is unreachable through a real install
        // and grants nothing if it is somehow reached (fail-closed).
        _ => &[],
    }
}

/// Grant each `[[tools]]`-declared role-tier cap (∩ `granted`) to its tier's role subjects in `ws`.
pub async fn grant_role_tiers(store: &Store, ws: &str, manifest: &Manifest, granted: &[String]) {
    for tool in &manifest.tools {
        let Some(tier) = tool.role.as_deref() else {
            continue;
        };
        let cap = format!(
            "mcp:{}:call",
            crate::ui_decl::qualify_tool(&manifest.id, &tool.name)
        );
        // Only grant what the install actually granted — never widen beyond `requested ∩ approved`.
        if !granted.iter().any(|g| g == &cap) {
            continue;
        }
        for role in roles_for(tier) {
            let subject = Subject::Role((*role).to_string());
            if let Err(e) = grant_assign(store, ws, &subject, &cap).await {
                eprintln!(
                    "grant_role_tiers: {} → {cap} for role:{role} skipped ({e})",
                    manifest.id
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"
[extension]
id = "estate"
version = "0.1.0"
[runtime]
tier = "wasm"
world = "lazybones:ext/extension@0.1.0"
placement = "either"
[[tools]]
name = "waste.summary"
role = "viewer"
[[tools]]
name = "waste.entry.upsert_month"
role = "admin"
[[tools]]
name = "waste.untiered"
[visibility]
class = "private"
"#;

    async fn caps_of(store: &Store, role: &str) -> Vec<String> {
        lb_authz::grant_list(store, "ws", &Subject::Role(role.into()))
            .await
            .unwrap()
    }

    /// The tiers land on the QUALIFIED cap the host gates on — the bare `[[tools]]` name never
    /// matched the install's `granted` set, so before the fix nothing was granted at all.
    #[tokio::test]
    async fn a_bare_tool_name_is_granted_under_its_qualified_cap() {
        let store = Store::memory().await.unwrap();
        let manifest = Manifest::parse(MANIFEST).unwrap();
        let granted: Vec<String> = [
            "mcp:estate.waste.summary:call",
            "mcp:estate.waste.entry.upsert_month:call",
            "mcp:estate.waste.untiered:call",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        grant_role_tiers(&store, "ws", &manifest, &granted).await;

        assert_eq!(
            caps_of(&store, "viewer").await,
            ["mcp:estate.waste.summary:call"]
        );
        assert_eq!(
            caps_of(&store, "member").await,
            ["mcp:estate.waste.summary:call"]
        );
        assert_eq!(
            caps_of(&store, "workspace-admin").await,
            [
                "mcp:estate.waste.entry.upsert_month:call",
                "mcp:estate.waste.summary:call"
            ]
        );
    }

    /// Never past what the install approved: a declared tier whose cap was not granted adds nothing.
    #[tokio::test]
    async fn a_tier_never_widens_past_the_granted_set() {
        let store = Store::memory().await.unwrap();
        let manifest = Manifest::parse(MANIFEST).unwrap();
        grant_role_tiers(&store, "ws", &manifest, &[]).await;
        assert!(caps_of(&store, "viewer").await.is_empty());
        assert!(caps_of(&store, "workspace-admin").await.is_empty());
    }

    #[test]
    fn tiers_nest_downward() {
        assert_eq!(roles_for("viewer"), ["viewer", "member", "workspace-admin"]);
        assert_eq!(roles_for("member"), ["member", "workspace-admin"]);
        assert_eq!(roles_for("admin"), ["workspace-admin"]);
        assert!(roles_for("bogus").is_empty(), "unknown tier grants nothing");
    }
}
