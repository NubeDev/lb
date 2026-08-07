//! The `/admin/invites*` lifecycle behaviours (invite-admin-routes scope): secret hygiene on the
//! roster, revoke's `204`/`404` contract, resend rotating the token, and the `?status=` filter. The
//! mandatory capability-deny / workspace-isolation / round-trip categories live in
//! `invite_admin_routes_test.rs`; shared fixtures in `common/invites.rs`. Split from that file to
//! stay under the FILE-LAYOUT 400-line limit.
//!
//! Same posture: the REAL router over a REAL embedded node, no mocks (CLAUDE §9). The gateway clock
//! is pinned at `NOW = 1000`, so an `expires_ts` below it is already past and one above it is live —
//! which is what makes the stored-vs-effective status case testable at all.

mod common;

use axum::http::StatusCode;
use common::invites::{hash_of, mint, roster, BOTH};
use common::{bearer, gateway, get_req, json_body, json_post, post_empty, token, NOW};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

// ---------------------------------------------------------------------------------------------
// Secret hygiene — the roster must never carry a redeemable token, for ANY status.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_roster_never_carries_a_redeemable_token_for_any_status() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);

    // One of each status: pending, accepted, revoked, and expired-but-stored-pending.
    let pending_raw = mint(&gw, &admin, json!({ "email": "pending@nube.com" })).await;
    let accept_raw = mint(&gw, &admin, json!({ "email": "accepted@nube.com" })).await;
    let revoke_raw = mint(&gw, &admin, json!({ "email": "revoked@nube.com" })).await;
    // The gateway clock is pinned at NOW=1000, so an expiry BELOW it is already past.
    let expired_raw = mint(
        &gw,
        &admin,
        json!({ "email": "expired@nube.com", "expires_ts": NOW - 500 }),
    )
    .await;

    let rows = roster(&gw, &admin, "").await;
    let revoked_hash = hash_of(&rows, "revoked@nube.com");
    let _ = router(gw.clone())
        .oneshot(bearer(
            post_empty(&format!("/admin/invites/{revoked_hash}/revoke")),
            &admin,
        ))
        .await
        .unwrap();
    let _ = router(gw.clone())
        .oneshot(json_post(
            "/public/invite/accept",
            json!({ "token": accept_raw, "workspace": "nube", "secret": "hunter2hunter2" }),
        ))
        .await
        .unwrap();

    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/invites"), &admin))
        .await
        .unwrap();
    let body = common::body_text(resp).await;
    for raw in [&pending_raw, &accept_raw, &revoke_raw, &expired_raw] {
        assert!(
            !body.contains(raw.as_str()),
            "the roster leaked a redeemable token"
        );
    }
    // Belt and braces: no field named like a token exists on any row at all.
    let rows: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_str(&body).unwrap();
    assert_eq!(rows.len(), 4);
    for row in &rows {
        assert!(row.contains_key("token_hash"), "the hash IS the address");
        assert!(!row.contains_key("token"), "no `token` field on a record");
    }
}

// ---------------------------------------------------------------------------------------------
// Revoke + resend semantics.
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_is_204_then_404_and_kills_the_token() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);
    let raw = mint(&gw, &admin, json!({ "email": "bob@nube.com" })).await;
    let hash = hash_of(&roster(&gw, &admin, "").await, "bob@nube.com");

    let resp = router(gw.clone())
        .oneshot(bearer(
            post_empty(&format!("/admin/invites/{hash}/revoke")),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NO_CONTENT,
        "a real revoke is 204"
    );

    // Idempotent at the host, but "nothing matched" reads as not-found at the route.
    let resp = router(gw.clone())
        .oneshot(bearer(
            post_empty(&format!("/admin/invites/{hash}/revoke")),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "a second revoke matches nothing → 404"
    );

    // An unknown hash is the SAME 404 — no revoked-vs-missing oracle.
    let resp = router(gw.clone())
        .oneshot(bearer(post_empty("/admin/invites/nope/revoke"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);

    // The revoked token no longer redeems.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/public/invite/accept",
            json!({ "token": raw, "workspace": "nube", "secret": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a revoked token must not redeem"
    );
    assert_eq!(roster(&gw, &admin, "?status=revoked").await.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resend_mints_a_new_token_and_invalidates_the_prior_one() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);
    let old_raw = mint(&gw, &admin, json!({ "email": "bob@nube.com" })).await;
    let hash = hash_of(&roster(&gw, &admin, "").await, "bob@nube.com");

    let resp = router(gw.clone())
        .oneshot(bearer(
            post_empty(&format!("/admin/invites/{hash}/resend")),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let out: serde_json::Value = json_body(resp).await;
    let new_raw = out["token"].as_str().expect("a fresh token").to_string();
    assert_ne!(new_raw, old_raw, "resend rotates the token");

    // The OLD link is dead.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/public/invite/accept",
            json!({ "token": old_raw, "workspace": "nube", "secret": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "the pre-resend token must be dead"
    );

    // The NEW link works.
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/public/invite/accept",
            json!({ "token": new_raw, "workspace": "nube", "secret": "hunter2hunter2" }),
        ))
        .await
        .unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "the resent token must redeem"
    );

    // Resending an already-redeemed invite is the uniform 404.
    let resp = router(gw.clone())
        .oneshot(bearer(post_empty("/admin/invites/nope/resend"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------------------------
// The `?status=` filter (the scope's open question #1, shipped from day one).
// ---------------------------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn status_filter_treats_a_lapsed_pending_record_as_expired() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);

    // `status` is STORED, not derived: this record is written `pending` and stays that way until
    // someone tries to redeem it. Past its expiry it must NOT be offered as pending.
    mint(
        &gw,
        &admin,
        json!({ "email": "lapsed@nube.com", "expires_ts": NOW - 1 }),
    )
    .await;
    mint(
        &gw,
        &admin,
        json!({ "email": "live@nube.com", "expires_ts": NOW + 10_000 }),
    )
    .await;
    mint(&gw, &admin, json!({ "email": "forever@nube.com" })).await; // expires_ts 0 = never

    let all = roster(&gw, &admin, "").await;
    assert_eq!(all.len(), 3, "the unfiltered roster is everything");
    assert!(
        all.iter().all(|r| r["status"] == "pending"),
        "all three are STORED pending: {all:?}"
    );

    let pending = roster(&gw, &admin, "?status=pending").await;
    let emails: Vec<&str> = pending
        .iter()
        .map(|r| r["email"].as_str().unwrap())
        .collect();
    assert_eq!(
        emails.len(),
        2,
        "only the redeemable ones are pending: {emails:?}"
    );
    assert!(emails.contains(&"live@nube.com") && emails.contains(&"forever@nube.com"));

    let expired = roster(&gw, &admin, "?status=expired").await;
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0]["email"], "lapsed@nube.com");

    assert!(roster(&gw, &admin, "?status=accepted").await.is_empty());
    assert!(roster(&gw, &admin, "?status=revoked").await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_status_value_is_a_400_not_an_empty_list() {
    let (gw, key) = gateway().await;
    let admin = token(&key, "user:test", "nube", BOTH);
    let resp = router(gw.clone())
        .oneshot(bearer(get_req("/admin/invites?status=banana"), &admin))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
