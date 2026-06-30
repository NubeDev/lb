//! Gateway integration for the Access console's three new verbs (access-console scope): the
//! resolved-effective-caps-with-provenance read, the live-token revoke lever, and roles.delete —
//! plus the **headline** behavior, that `revoke_tokens` makes the subject's prior token refused on
//! the next verify (single-node = instant). Real node, real store, real gateway — no mocks. Mirrors
//! `admin_routes_test.rs`'s shape (a forged non-admin call is denied server-side; the UI gate is not
//! the boundary).

mod common;

use axum::http::StatusCode;
use common::{bearer, body_text, delete_req, gateway, get_req, json_body, json_post, token};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// The admin cap set this slice adds, plus the grants/roles caps the seed steps need.
const ADMIN: &[&str] = &[
    "mcp:grants.assign:call",
    "mcp:grants.list:call",
    "mcp:roles.define:call",
    "mcp:roles.list:call",
    "mcp:roles.manage:call",
    "mcp:teams.manage:call",
    "mcp:teams.list:call",
    "mcp:members.add:call",
    "mcp:authz.resolve:call",
    "mcp:authz.revoke-tokens:call",
    // a cap the admin holds so no-widening is satisfied when granting it to bob.
    "mcp:hvac.setpoint:call",
];

// ── Mandatory: capability deny, per new verb, over the real gateway ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn forged_call_by_non_admin_is_denied_server_side() {
    let (gw, key) = gateway().await;
    // A valid session token holding NO access-console caps.
    let tok = token(&key, "user:mallory", "acme", &["bus:chan/*:pub"]);

    for req in [
        get_req("/admin/authz/resolve?subject=user:bob"),
        json_post(
            "/admin/authz/revoke-tokens",
            json!({ "subject": "user:bob" }),
        ),
        delete_req("/admin/roles/operator"),
    ] {
        let resp = router(gw.clone()).oneshot(bearer(req, &tok)).await.unwrap();
        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "forged access-console call must be 403 server-side"
        );
    }
}

// ── resolve: resolved effective caps WITH provenance ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_returns_sourced_caps_direct_role_and_team() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:alice", "acme", ADMIN);

    // bob gets a DIRECT grant.
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    // a role bundling a store cap, assigned to the `facilities` team.
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "operator", "caps": ["mcp:hvac.setpoint:call"] }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/teams",
                json!({ "team": "facilities", "name": "Facilities" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "team:facilities", "cap": "role:operator" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    // bob joins facilities (the live membership edge the resolver walks).
    router(gw.clone())
        .oneshot(bearer(
            json_post("/teams/facilities/members", json!({ "user": "bob" })),
            &admin,
        ))
        .await
        .unwrap();

    // resolve bob → the hvac cap appears, sourced BOTH `direct` AND `via team facilities`.
    let resp = router(gw.clone())
        .oneshot(bearer(
            get_req("/admin/authz/resolve?subject=user:bob"),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let caps: Vec<Value> = json_body(resp).await;
    let hvac = caps
        .iter()
        .find(|c| c["cap"] == "mcp:hvac.setpoint:call")
        .expect("hvac cap resolved");
    let kinds: Vec<&str> = hvac["source"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap())
        .collect();
    assert!(
        kinds.contains(&"direct") && kinds.contains(&"team"),
        "hvac must be sourced direct + via-team; got {kinds:?}"
    );

    // A subject bob is NOT (eve) resolves empty — no fabrication.
    let resp = router(gw)
        .oneshot(bearer(
            get_req("/admin/authz/resolve?subject=user:eve"),
            &admin,
        ))
        .await
        .unwrap();
    let caps: Vec<Value> = json_body(resp).await;
    assert!(
        caps.is_empty(),
        "eve has no caps — honest empty, not a fake"
    );
}

// ── THE HEADLINE: revoke_tokens refuses the subject's prior token on the next verify ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_tokens_refuses_bobs_prior_token_on_next_verify() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:alice", "acme", ADMIN);
    // bob holds a real, valid (unexpired) session token, including the read cap the probe uses.
    let bob_tok = token(
        &key,
        "user:bob",
        "acme",
        &["mcp:hvac.setpoint:call", "mcp:workspace.list:call"],
    );

    // bob's token works BEFORE the revoke (a normal authenticated read succeeds).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/workspaces"), &bob_tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "bob's token works pre-revoke"
    );

    // Admin applies the live-token revoke lever for bob (marker + grant-revoke).
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/authz/revoke-tokens",
                json!({ "subject": "user:bob" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert!(
        out["grants_revoked"].as_u64().is_some(),
        "reports the consequence count"
    );

    // bob's SAME prior token is now REFUSED on the next verify — the marker bit (single-node =
    // instant). 401, indistinguishable from a genuinely expired credential (no oracle).
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/workspaces"), &bob_tok))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::UNAUTHORIZED,
        "bob's live token must be refused after revoke_tokens"
    );

    // A FRESHLY minted token for a different subject (carol) still works — the marker is per-
    // subject, not a global lockout.
    let carol_tok = token(&key, "user:carol", "acme", &["mcp:workspace.list:call"]);
    let resp = router(gw)
        .oneshot(bearer(get_req("/workspaces"), &carol_tok))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "carol is unaffected");
}

// ── roles.delete: cascade + built-in immutable + idempotent ──────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn roles_delete_cascades_and_builtins_are_rejected() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:alice", "acme", ADMIN);

    // Define `operator` and assign it to two subjects.
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "operator", "caps": ["mcp:hvac.setpoint:call"] }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    for sub in ["user:bob", "team:facilities"] {
        router(gw.clone())
            .oneshot(bearer(
                json_post(
                    "/admin/grants",
                    json!({ "subject": sub, "cap": "role:operator" }),
                ),
                &admin,
            ))
            .await
            .unwrap();
    }

    // Delete → 200 with affected = 2 (both role grants tombstoned in one tx).
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/admin/roles/operator"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(
        out["affected"],
        json!(2),
        "cascade un-assigns both subjects"
    );

    // Idempotent: deleting again is a no-op success (role gone, 0 assignees).
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/admin/roles/operator"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(out["affected"], json!(0));

    // Built-in roles are immutable → 400 with a clear reason (never an opaque 403).
    let resp = router(gw)
        .oneshot(bearer(delete_req("/admin/roles/member"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let txt = body_text(resp).await;
    assert!(
        txt.contains("immutable"),
        "built-in reject states why: {txt}"
    );
}

// ── Mandatory: two-workspace isolation. The wall is token-derived (ws comes from the token, never
//    the request), so a ws-B admin's resolve/revoke/delete CANNOT target ws-A: they operate in
//    ws-B (globex), resolve-empty / delete-nothing there, and leave acme's state intact. ────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_touch_ws_a_access_state() {
    let (gw, key) = gateway().await;
    let admin_a = token(&key, "user:alice", "acme", ADMIN);
    // ws-B admin: full caps, but in workspace `globex`.
    let admin_b = token(&key, "user:carol", "globex", ADMIN);

    // ws-A seeds a role + a grant in acme.
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/roles",
                json!({ "name": "operator", "caps": ["mcp:hvac.setpoint:call"] }),
            ),
            &admin_a,
        ))
        .await
        .unwrap();
    router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "user:bob", "cap": "mcp:hvac.setpoint:call" }),
            ),
            &admin_a,
        ))
        .await
        .unwrap();
    // bob's acme token (carries the workspace.list cap so the probe reads through).
    let bob_acme = token(&key, "user:bob", "acme", &["mcp:workspace.list:call"]);

    // ws-B resolve of "user:bob" → 200 but EMPTY: the workspace is globex (from carol's token), so
    // acme's grant does NOT leak across the wall.
    let resp = router(gw.clone())
        .oneshot(bearer(
            get_req("/admin/authz/resolve?subject=user:bob"),
            &admin_b,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let caps: Vec<Value> = json_body(resp).await;
    assert!(
        caps.is_empty(),
        "bob has no caps in globex — the wall holds"
    );

    // ws-B revoke-tokens of "user:bob" lands in globex, NOT acme: bob's acme token still works.
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/authz/revoke-tokens",
                json!({ "subject": "user:bob" }),
            ),
            &admin_b,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "ws-B revoke operates in its own ws"
    );
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/workspaces"), &bob_acme))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "bob's acme token is unaffected — the marker landed in globex"
    );

    // ws-B roles.delete of "operator" deletes nothing in acme: the role is absent in globex (0
    // affected), and acme's operator role is still listed.
    let resp = router(gw.clone())
        .oneshot(bearer(delete_req("/admin/roles/operator"), &admin_b))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: Value = json_body(resp).await;
    assert_eq!(out["affected"], json!(0), "nothing to delete in globex");
    let resp = router(gw)
        .oneshot(bearer(get_req("/admin/roles"), &admin_a))
        .await
        .unwrap();
    let roles: Vec<Value> = json_body(resp).await;
    assert!(
        roles.iter().any(|r| r["name"] == "operator"),
        "acme's operator role survived the ws-B delete: {roles:?}"
    );
}
