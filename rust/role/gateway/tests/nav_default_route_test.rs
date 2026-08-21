//! `GET /nav/default` — the READ half of the workspace-default nav pointer, over the REAL gateway +
//! SurrealDB (no mocks, CLAUDE §9).
//!
//! **The bug this locks.** The pointer shipped write-only: `POST /nav/default` persisted, but nothing
//! could ever ask which nav the workspace points at. A builder UI could only badge the row it had
//! just written, in that browser session — reload, or open the builder as a second admin, and the
//! "Default" badge was gone while the default was still set server-side. Setting it also changes
//! nothing on an admin's OWN rail (admins skip tiers 2/3 by the no-lockout rule), so the write looked
//! like a no-op from both directions (rubix-ai#165).
//!
//! **The gate asymmetry is the design, and is asserted here:** the POST is admin-ish
//! (`mcp:nav.save:call`); the GET is member-level (`mcp:nav.resolve:call`), because the pointer is
//! already the third tier of the caller's own resolve. A plain member reads it and cannot set it.

mod common;

use axum::http::StatusCode;
use common::{bearer, gateway, get_req, json_post};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

/// `GET /nav/default` under `token` — the status plus the decoded body.
async fn get_default(gw: &Gateway, token: &str) -> (StatusCode, Value) {
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/nav/default"), token))
        .await
        .unwrap();
    let status = resp.status();
    let text = common::body_text(resp).await;
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    (status, body)
}

/// `POST /nav/default {id}` under `token` — the status only.
async fn post_default(gw: &Gateway, token: &str, id: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(bearer(
            json_post("/nav/default", json!({ "id": id })),
            token,
        ))
        .await
        .unwrap()
        .status()
}

/// Save a nav as `token` (setup); asserts it landed.
async fn save_nav(gw: &Gateway, token: &str, id: &str, title: &str) {
    let resp = router(gw.clone())
        .oneshot(bearer(
            json_post(
                "/navs",
                json!({
                    "id": id,
                    "title": title,
                    "items": [ { "kind": "surface", "surface": "dashboards", "label": "Home" } ]
                }),
            ),
            token,
        ))
        .await
        .unwrap();
    assert!(
        resp.status().is_success(),
        "setup: saving nav `{id}` should succeed, got {}",
        resp.status()
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn default_pointer_round_trips_over_the_route() {
    let (gw, _key) = gateway().await;
    let admin = common::bootstrap::provision_admin(&gw, "user:alice", "nube").await;

    // Nothing set yet — an explicit `null`, not a 404. "No default" is a real answer a UI renders
    // (no badge on any row), not an error it has to swallow.
    let (status, body) = get_default(&gw, &admin).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "id": Value::Null }));

    save_nav(&gw, &admin, "ops", "Ops").await;
    assert_eq!(
        post_default(&gw, &admin, "ops").await,
        StatusCode::NO_CONTENT
    );

    // The read names what the write set — the durable badge the builder could not have before.
    let (status, body) = get_default(&gw, &admin).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "id": "ops" }));

    // Clearing reads back as cleared (the tombstone shape), not as the stale pointer.
    assert_eq!(post_default(&gw, &admin, "").await, StatusCode::NO_CONTENT);
    let (_, body) = get_default(&gw, &admin).await;
    assert_eq!(body, json!({ "id": Value::Null }));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_read_is_member_level_and_the_write_stays_admin_ish() {
    let (gw, _key) = gateway().await;
    let admin = common::bootstrap::provision_admin(&gw, "user:alice", "nube").await;
    save_nav(&gw, &admin, "ops", "Ops").await;
    assert_eq!(
        post_default(&gw, &admin, "ops").await,
        StatusCode::NO_CONTENT
    );

    // bob is a plain member — `role:member` carries `mcp:nav.resolve:call`, not `mcp:nav.save:call`.
    let bob = common::bootstrap::provision_member(&gw, "user:bob", "nube").await;
    let (status, body) = get_default(&gw, &bob).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a member may READ the pointer that already shapes their own menu"
    );
    assert_eq!(body, json!({ "id": "ops" }));

    assert_eq!(
        post_default(&gw, &bob, "other").await,
        StatusCode::FORBIDDEN,
        "…and may not SET it — the write keeps the authoring cap"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_default_pointer_is_workspace_walled() {
    let (gw, _key) = gateway().await;
    let nube = common::bootstrap::provision_admin(&gw, "user:alice", "nube").await;
    save_nav(&gw, &nube, "ops", "Ops").await;
    assert_eq!(
        post_default(&gw, &nube, "ops").await,
        StatusCode::NO_CONTENT
    );

    // A ws-B admin reads ws-B's own (unset) pointer — nube's default is structurally invisible (§7).
    let beta = common::bootstrap::provision_admin(&gw, "user:carol", "beta").await;
    let (status, body) = get_default(&gw, &beta).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "id": Value::Null }));
}
