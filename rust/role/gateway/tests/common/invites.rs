//! Shared fixtures for the `/admin/invites*` route suites (invite-admin-routes scope), split across
//! `invite_admin_routes_test.rs` (the mandatory categories) and `invite_admin_lifecycle_test.rs`
//! (revoke/resend/hygiene/filter) to stay under the FILE-LAYOUT 400-line limit. Both drive the REAL
//! router, so these helpers are thin request wrappers — never a re-implementation of route
//! behaviour.
#![allow(dead_code)]

use axum::http::StatusCode;
use lb_role_gateway::{router, Gateway};
use tower::ServiceExt;

use super::{bearer, get_req, json_body, json_post};

/// The cap gating mint + revoke + resend.
pub const CREATE: &str = "mcp:invite.create:call";
/// The cap gating the roster read — a DIFFERENT cap (`builtin_roles.rs`), which is the whole point
/// of the deny tests.
pub const LIST: &str = "mcp:invite.list:call";
/// What the `workspace-admin` bundle actually holds: both.
pub const BOTH: &[&str] = &[CREATE, LIST];

/// Mint an invite through the real route and return the raw one-time token. The `token_hash` the
/// other routes address the record by is read back off the roster ([`hash_of`]), never derived here
/// — that is exactly the round trip a console makes.
pub async fn mint(gw: &Gateway, tok: &str, body: serde_json::Value) -> String {
    let resp = router(gw.clone())
        .oneshot(bearer(json_post("/admin/invites", body), tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "mint must succeed");
    let out: serde_json::Value = json_body(resp).await;
    out["token"].as_str().expect("token in reply").to_string()
}

/// The roster for the caller's workspace; `q` is the raw query suffix (e.g. `"?status=pending"`).
pub async fn roster(gw: &Gateway, tok: &str, q: &str) -> Vec<serde_json::Value> {
    let resp = router(gw.clone())
        .oneshot(bearer(get_req(&format!("/admin/invites{q}")), tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "list must succeed");
    json_body(resp).await
}

/// The `token_hash` of the roster row for `email`.
pub fn hash_of(rows: &[serde_json::Value], email: &str) -> String {
    rows.iter()
        .find(|r| r["email"] == email)
        .unwrap_or_else(|| panic!("no invite row for {email}"))["token_hash"]
        .as_str()
        .expect("token_hash is a string")
        .to_string()
}
