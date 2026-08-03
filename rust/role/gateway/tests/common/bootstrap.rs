//! Provision a **first workspace admin** in a test, without a login route.
//!
//! The legacy `POST /login` used to double as a bootstrap: the first login into an EMPTY workspace
//! silently made the requester a `workspace-admin` (`membership_login_resolve`). That route and that
//! bootstrap were deleted in the pre-production legacy sweep (email-login scope) — a stranger who can
//! reach the port must never be able to promote themselves — so the first admin is now **explicitly
//! provisioned by the operator**, exactly as the scope's "first-admin story" specifies.
//!
//! This helper is that provisioning, in test form: it runs the SAME un-gated seams `seed_dev_identity`
//! runs at boot (workspace directory register → built-in role records → identity → membership row →
//! `member` + `workspace-admin` grants) against the REAL store, then mints through the SAME
//! role-correct path every `/auth/*` route mints through (`mint_full_session`). No mocks, no fake
//! caps list: the token carries whatever the durable grants resolve to, so a test that asserts an
//! admin can do something is asserting the real grant chain.

use lb_authz as raw;
use lb_role_gateway::Gateway;

use super::NOW;

/// Provision `sub` as a `workspace-admin` member of `ws` on `gw`'s node and return a real signed
/// session token for them. Idempotent — every write below is an upsert.
pub async fn provision_admin(gw: &Gateway, sub: &str, ws: &str) -> String {
    let store = &gw.node.store;
    // The workspace must be in the node DIRECTORY, or `login_workspaces`/`identity.workspaces` find
    // nothing to scan (the directory register used to happen lazily on the first legacy login).
    lb_host::workspace_register(store, ws, ws, NOW)
        .await
        .expect("register workspace in the directory");
    // The built-in `member`/`workspace-admin` role RECORDS, so the grants below resolve to caps.
    lb_host::ensure_builtin_authz_roles(store, ws)
        .await
        .expect("seed built-in roles");
    raw::identity_create(store, sub, None, NOW)
        .await
        .expect("create identity");
    raw::membership_add_raw(store, ws, sub, NOW)
        .await
        .expect("write membership row");
    let bare = sub.strip_prefix("user:").unwrap_or(sub);
    let subject = raw::Subject::User(bare.to_string());
    raw::grant_assign(store, ws, &subject, "role:member")
        .await
        .expect("grant role:member");
    raw::grant_assign(store, ws, &subject, "role:workspace-admin")
        .await
        .expect("grant role:workspace-admin");

    lb_role_gateway::mint_full_session(&gw.node, &gw.key, sub, ws, NOW)
        .await
        .token
}

/// Mint a role-correct session token for a `sub` that is ALREADY a member (e.g. one an admin just
/// added over `POST /admin/members`). No seeding — this is purely "what would `/auth/login` hand
/// this person?", through the same `mint_full_session` the route uses, so the caps are the durable
/// grants and nothing else.
pub async fn session_token(gw: &Gateway, sub: &str, ws: &str) -> String {
    lb_role_gateway::mint_full_session(&gw.node, &gw.key, sub, ws, NOW)
        .await
        .token
}

/// Provision `sub` as a plain **member** of `ws` (no admin role) and return their session token.
/// Same real seams as [`provision_admin`], minus the `workspace-admin` grant.
pub async fn provision_member(gw: &Gateway, sub: &str, ws: &str) -> String {
    let store = &gw.node.store;
    lb_host::workspace_register(store, ws, ws, NOW)
        .await
        .expect("register workspace in the directory");
    lb_host::ensure_builtin_authz_roles(store, ws)
        .await
        .expect("seed built-in roles");
    raw::identity_create(store, sub, None, NOW)
        .await
        .expect("create identity");
    raw::membership_add_raw(store, ws, sub, NOW)
        .await
        .expect("write membership row");
    let bare = sub.strip_prefix("user:").unwrap_or(sub);
    raw::grant_assign(
        store,
        ws,
        &raw::Subject::User(bare.to_string()),
        "role:member",
    )
    .await
    .expect("grant role:member");

    lb_role_gateway::mint_full_session(&gw.node, &gw.key, sub, ws, NOW)
        .await
        .token
}
