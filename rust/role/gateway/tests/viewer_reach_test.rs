//! Viewer-tier reach — the **nav-as-reach** regression, over the REAL gateway + SurrealDB (no mocks,
//! CLAUDE §9). This is the live finding the branch set out to fix: a user given a one-page NAV could
//! still open the Rules editor by URL, because `mcp:rules.*` was a MEMBER cap and the *cap gate* (the
//! real reach boundary) passed — the nav is a pure lens and never gated reach (access-model scope).
//!
//! The fix is the three-tier split (`viewer ⊂ member ⊂ admin`): the author surface (rules/flows/
//! queries/templates/datasource registration + the broad write wildcards) moved out of the base
//! bundle into the AUTHOR delta a `member` holds, and the login FLOOR is now the `viewer` set — so a
//! `viewer`-role token is never re-widened to a member. A viewer given a curated nav genuinely cannot
//! reach an authoring page: the server-side cap gate `403`s it. This test proves exactly that:
//!
//! (a) **A member authors** — the baseline that must stay green: a plain member `200`s `rules.save`
//!     (we did not break authoring for real members).
//! (b) **A viewer cannot author** — after the admin regrades bob to `role:viewer`, the SAME
//!     authoring verbs (`rules.save`, `flows.save`, `query.run`, `template.save`, `dashboard.save`,
//!     `datasource.add`) all `403` at the `/mcp/call` bridge — the reach a one-page nav must withhold.
//! (c) **A viewer keeps view reach** — the same viewer token still `200`s the render/read path
//!     (`dashboard.list`), so a curated nav renders the pages it WAS given. Restricting reach ≠
//!     breaking the view.
//!
//! (d) **The deny is opaque even for malformed args** — the gate runs before the schema validator,
//!     so a denied caller cannot use a schema-declaring verb as a shape oracle. This one guards the
//!     others: while the validator ran first, `dashboard.save` in (b) returned `400` and that row
//!     silently stopped asserting the deny at all.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, get_req, json_post};
use lb_role_gateway::{router, Gateway};
use serde_json::json;
use tower::ServiceExt;

/// The cap gate must run BEFORE the schema validator, so a denied caller can never tell a
/// well-formed call from a malformed one. `dashboard.save` is the probe: it is one of the few verbs
/// that declares an `input_schema` (`required: [id, title, cells, now]`), so it is exactly the tier
/// that leaks if the validator runs first. A viewer sending args that violate that schema must still
/// get the opaque `403` — never the `400` that would confirm the verb exists and grade their args.
///
/// Regression: the validator sat ahead of `authorize_tool` in `dispatch_at_depth`, so this returned
/// `400` and the deny below silently stopped testing the deny (see the `denied` table in (b)).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_denied_caller_gets_an_opaque_403_even_when_the_args_are_malformed() {
    let (gw, _key) = gateway().await;
    let viewer = seed_viewer(&gw).await;

    // Missing `now` AND `cells` — a schema violation the validator would reject with 400.
    let status = mcp_status(&gw, &viewer, "dashboard.save", json!({ "id": "d1" })).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a denied viewer must get an opaque 403, not a 400 grading the args of a verb it cannot call"
    );

    // The same call with args that SATISFY the schema is also 403 — the two are indistinguishable,
    // which is the whole point of the contract.
    let status = mcp_status(
        &gw,
        &viewer,
        "dashboard.save",
        json!({ "id": "d1", "title": "d1", "cells": [], "now": 1_000_u64 }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "well-formed args must be denied identically — no shape oracle"
    );
}

/// Drive the REAL admin routes to produce a `viewer`-role token for `user:bob`: alice bootstraps as
/// workspace-admin, adds bob, grants `role:viewer`, revokes `role:member`. Bob re-logs in so his
/// token carries the viewer floor ∪ his one remaining role. Returns bob's viewer bearer.
async fn seed_viewer(gw: &Gateway) -> String {
    let admin = login(gw, "user:alice", "acme").await;
    let post = |path: &'static str, body: serde_json::Value, tok: String| {
        let gw = gw.clone();
        async move {
            router(gw)
                .oneshot(bearer(json_post(path, body), &tok))
                .await
                .unwrap()
                .status()
        }
    };
    assert_eq!(
        post(
            "/admin/members",
            json!({ "sub": "user:bob" }),
            admin.clone()
        )
        .await,
        StatusCode::NO_CONTENT,
        "admin adds bob"
    );
    assert_eq!(
        post(
            "/admin/grants",
            json!({ "subject": "user:bob", "cap": "role:viewer" }),
            admin.clone()
        )
        .await,
        StatusCode::NO_CONTENT,
        "admin grants bob role:viewer"
    );
    assert_eq!(
        post(
            "/admin/grants/revoke",
            json!({ "subject": "user:bob", "cap": "role:member" }),
            admin
        )
        .await,
        StatusCode::NO_CONTENT,
        "admin revokes bob role:member"
    );
    login(gw, "user:bob", "acme").await
}

/// Log in over the real `/login` route (password-less dev check) and return the bearer token.
async fn login(gw: &Gateway, user: &str, ws: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login {user}@{ws} ok");
    let reply: serde_json::Value = common::json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

/// Call `tool` with `args` over the real `/mcp/call` bridge under `token`; return the HTTP status.
/// The bridge re-checks `mcp:<tool>:call` before dispatch — an ungranted verb is `403` (opaque),
/// which is precisely the reach gate a viewer must hit on an authoring verb.
async fn mcp_status(gw: &Gateway, token: &str, tool: &str, args: serde_json::Value) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(
            json_post("/mcp/call", json!({ "tool": tool, "args": args })),
            token,
        ))
        .await
        .unwrap()
        .status()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_viewer_cannot_reach_authoring_pages_but_a_member_can_and_a_viewer_still_views() {
    let (gw, _key) = gateway().await;

    // alice bootstraps as workspace-admin (first login into an empty ws); she adds bob as a member.
    let admin = login(&gw, "user:alice", "acme").await;
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "admin adds bob");

    // ── (a) A MEMBER authors: bob (role:member) can save a rule. Baseline that must stay green. ──
    let member = login(&gw, "user:bob", "acme").await;
    let status = mcp_status(
        &gw,
        &member,
        "rules.save",
        json!({ "id": "r1", "name": "r1", "body": "true" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a MEMBER authors — rules.save 200s (we did not break authoring for real members)"
    );

    // ── Regrade bob to a VIEWER: grant role:viewer, revoke role:member (both admin acts). ──
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants",
                json!({ "subject": "user:bob", "cap": "role:viewer" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "admin grants bob role:viewer"
    );
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/admin/grants/revoke",
                json!({ "subject": "user:bob", "cap": "role:member" }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "admin revokes bob role:member"
    );

    // Bob re-logs in → a VIEWER token (viewer floor ∪ his only remaining role = viewer caps).
    let viewer = login(&gw, "user:bob", "acme").await;

    // ── (b) A VIEWER cannot author: every authoring verb the nav must withhold → 403 at the gate. ──
    // Each is the exact reach a one-page nav could never restrict before — the cap gate now denies it.
    let denied: &[(&str, serde_json::Value)] = &[
        (
            "rules.save",
            json!({ "id": "r2", "name": "r2", "body": "true" }),
        ),
        ("rules.run", json!({ "id": "r1" })),
        ("flows.save", json!({ "id": "f1", "graph": {} })),
        ("query.run", json!({ "id": "q1" })),
        (
            "template.save",
            json!({ "id": "t1", "name": "t1", "body": "x" }),
        ),
        (
            "dashboard.save",
            json!({ "id": "d1", "title": "d1", "cells": [] }),
        ),
        (
            "datasource.add",
            json!({ "uid": "s1", "kind": "sqlite", "dsn": "/tmp/x.db" }),
        ),
    ];
    for (tool, args) in denied {
        let status = mcp_status(&gw, &viewer, tool, args.clone()).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "a VIEWER must be denied the authoring verb `{tool}` (the reach a one-page nav withholds)"
        );
    }

    // ── (c) A VIEWER still views: the render/read path stays open, so a curated nav renders. ──
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/dashboards"), &viewer))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "a VIEWER keeps view reach — dashboard.list still 200s (restricting reach ≠ breaking the view)"
    );
}
