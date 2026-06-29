//! api-keys scope — the full lifecycle over the REAL gateway (no mocks, real node + real store + real
//! caps). Covers the mandatory categories: capability-deny (per management verb), the
//! privilege-escalation deny (incl. the built-in-role path), read-only/read-write cap enforcement at
//! the chokepoint, workspace isolation, lazy-expiry boundary, revoke tombstone idempotency, the
//! create→auth→allowed→denied→revoke→refused flow, rotate (old dead / new works), and the cache
//! busting on revoke (refused immediately, not after the TTL). Plus the list/get-no-secret assertion.

mod common;

use axum::body::Body;
use common::{bearer, gateway, get_req, json_body, json_post, post_empty, token, NOW};
use lb_role_gateway::router;
use serde_json::{json, Value};
use tower::ServiceExt;

/// The admin cap set: `apikey.manage` + the built-in role cap bundles + the read probe cap. The dev
/// admin HOLDS the role caps so the no-widening guard lets it mint keys under either built-in role.
const ADMIN_CAPS: &[&str] = &[
    "mcp:apikey.manage:call",
    "store:*:read",
    "store:*:write",
    "mcp:*.get:call",
    "mcp:*.list:call",
    "mcp:*.write:call",
    "mcp:*.create:call",
    "mcp:*.update:call",
    "mcp:*.delete:call",
    "mcp:*.post:call",
    "mcp:outbox.status:call",
];

/// A non-admin cap set (lacks `apikey.manage`) — for the deny tests.
const NO_MANAGE: &[&str] = &["bus:chan/*:pub"];

/// Mint an admin JWT for `ws`.
fn admin(key: &lb_auth::SigningKey, ws: &str) -> String {
    token(key, "user:admin", ws, ADMIN_CAPS)
}

/// Create a key as the admin, returning the one-time bearer string.
async fn create_key(app: &axum::Router, admin_token: &str, body: Value) -> String {
    let resp = app
        .clone()
        .oneshot(bearer(json_post("/admin/apikeys", body), admin_token))
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK, "create failed");
    let created: Value = json_body(resp).await;
    created["key"].as_str().unwrap().to_string()
}

/// Extract the key id from a bearer `lbk_{ws}.{id}.{secret}`.
fn key_id(bearer: &str) -> &str {
    bearer
        .strip_prefix("lbk_")
        .unwrap()
        .split('.')
        .nth(1)
        .unwrap()
}

/// Call `POST /mcp/call` with a bearer credential and a `{tool, args}` body.
fn mcp_call(tool: &str, args: Value) -> Request<Body> {
    json_post("/mcp/call", json!({ "tool": tool, "args": args }))
}

use axum::http::Request;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn management_verbs_denied_without_apikey_manage() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = token(&key, "user:bob", "acme", NO_MANAGE);

    // list
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/apikeys"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::FORBIDDEN);
    // create
    let r = app
        .clone()
        .oneshot(bearer(
            json_post("/admin/apikeys", json!({"label":"x"})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::FORBIDDEN);
    // get / revoke / rotate on a (nonexistent) id — also denied before the not-found check
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/apikeys/k1"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::FORBIDDEN);
    let r = app
        .clone()
        .oneshot(bearer(post_empty("/admin/apikeys/k1/revoke"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn escalation_denied_when_effective_caps_widen_beyond_creator() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    // A narrow admin: has apikey.manage but LACKS store:*:write (and the write tool caps).
    let narrow = token(
        &key,
        "user:narrow",
        "acme",
        &["mcp:apikey.manage:call", "mcp:outbox.status:call"],
    );

    // Creating an apikey-write key (whose role bundles store:*:write) is REFUSED — the role path the
    // grants_assign exemption would otherwise miss.
    let r = app
        .clone()
        .oneshot(bearer(
            json_post("/admin/apikeys", json!({"label":"w","role":"apikey-write"})),
            &narrow,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::BAD_REQUEST);

    // A custom cap the creator lacks is also refused.
    let r = app
        .clone()
        .oneshot(bearer(
            json_post(
                "/admin/apikeys",
                json!({"label":"c","role":"apikey-read","caps":["store:secret:write"]}),
            ),
            &narrow,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_and_get_carry_no_hash_or_secret() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");
    let bearer_str = create_key(
        &app,
        &tok,
        json!({"label":"rooftop-hvac","kind":"appliance","role":"apikey-read"}),
    )
    .await;
    let id = key_id(&bearer_str);

    let resp = app
        .clone()
        .oneshot(bearer(get_req("/admin/apikeys"), &tok))
        .await
        .unwrap();
    let list: Value = json_body(resp).await;
    let list_str = list.to_string();
    assert!(
        !list_str.contains("key_hash") && !list_str.contains(bearer_str.split('.').nth(2).unwrap()),
        "list must not expose the hash or the secret"
    );
    let row = list
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .unwrap();
    assert_eq!(row["label"], "rooftop-hvac");
    assert_eq!(row["badge"], "read-only");

    let resp = app
        .clone()
        .oneshot(bearer(get_req(&format!("/admin/apikeys/{id}")), &tok))
        .await
        .unwrap();
    let full: Value = json_body(resp).await;
    let full_str = full.to_string();
    assert!(
        !full_str.contains("key_hash") && !full_str.contains(bearer_str.split('.').nth(2).unwrap()),
        "get must not expose the hash or the secret"
    );
    assert!(
        full["caps"].as_array().unwrap().len() > 0,
        "get resolves caps"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_auth_allow_deny_revoke_refused() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");
    // read-only role + the outbox.status probe cap (which the admin holds, so no-widening passes).
    let kbearer = create_key(
        &app,
        &tok,
        json!({"label":"ro","role":"apikey-read","caps":["mcp:outbox.status:call"]}),
    )
    .await;
    let id = key_id(&kbearer);

    // allowed call: the key's granted outbox.status
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &kbearer))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::OK,
        "granted call allowed"
    );

    // denied call: the read-only key lacks inbox.record
    let r = app
        .clone()
        .oneshot(bearer(
            mcp_call(
                "inbox.record",
                json!({"channel":"c","id":"i","body":"b","ts":1}),
            ),
            &kbearer,
        ))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::FORBIDDEN,
        "read-only denied write"
    );

    // revoke
    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/apikeys/{id}/revoke")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::NO_CONTENT);

    // refused next request — the cache was populated by the allowed call above; revoke busted it, so
    // the next auth misses and reads the tombstone. The clock is UNCHANGED (NOW), so a missing bust
    // would still serve the cached principal within the 5s TTL → this asserts the bust is immediate.
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &kbearer))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "revoked key refused"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_is_idempotent() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");
    let kbearer = create_key(&app, &tok, json!({"label":"k"})).await;
    let id = key_id(&kbearer);

    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/apikeys/{id}/revoke")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::NO_CONTENT);
    // re-revoke is a no-op success (the tombstone upsert is idempotent — the offline/sync property).
    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/apikeys/{id}/revoke")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::NO_CONTENT);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rotate_kills_old_secret_new_works() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");
    let old = create_key(
        &app,
        &tok,
        json!({"label":"rot","role":"apikey-read","caps":["mcp:outbox.status:call"]}),
    )
    .await;
    let id = key_id(&old);

    // old works
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &old))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::OK);

    // rotate → new bearer
    let resp = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/apikeys/{id}/rotate")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), axum::http::StatusCode::OK);
    let new: Value = json_body(resp).await;
    let new_bearer = new["key"].as_str().unwrap().to_string();
    assert_ne!(new_bearer, old, "rotation produces a new secret");

    // old dead instantly
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &old))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "old secret dead"
    );
    // new works
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &new_bearer))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::OK, "new secret works");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn lazy_expiry_boundary_now_equals_and_exceeds_expires_at() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    // expires_at == NOW → already expired at the fixed clock NOW.
    let exp_now = create_key(
        &app,
        &tok,
        json!({"label":"e-now","role":"apikey-read","caps":["mcp:outbox.status:call"],"expires_at":NOW}),
    )
    .await;
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &exp_now))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "now==expires_at refused"
    );

    // expires_at < NOW → expired.
    let exp_past = create_key(
        &app,
        &tok,
        json!({"label":"e-past","role":"apikey-read","caps":["mcp:outbox.status:call"],"expires_at":NOW-1}),
    )
    .await;
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &exp_past))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "now>expires_at refused"
    );

    // expires_at > NOW → valid.
    let exp_future = create_key(
        &app,
        &tok,
        json!({"label":"e-fut","role":"apikey-read","caps":["mcp:outbox.status:call"],"expires_at":NOW+100}),
    )
    .await;
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &exp_future))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::OK,
        "now<expires_at allowed"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_ws_b_cannot_see_or_use_ws_a_keys() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let admin_a = admin(&key, "ws-a");
    let admin_b = admin(&key, "ws-b");

    // mint a key in ws A
    let a_bearer = create_key(
        &app,
        &admin_a,
        json!({"label":"a-key","role":"apikey-read","caps":["mcp:outbox.status:call"]}),
    )
    .await;
    let a_id = key_id(&a_bearer);

    // ws B's list never contains ws A's key
    let resp = app
        .clone()
        .oneshot(bearer(get_req("/admin/apikeys"), &admin_b))
        .await
        .unwrap();
    let list: Value = json_body(resp).await;
    assert!(
        !list.to_string().contains(a_id),
        "ws B must not list ws A's keys"
    );

    // ws B cannot get ws A's key
    let r = app
        .clone()
        .oneshot(bearer(get_req(&format!("/admin/apikeys/{a_id}")), &admin_b))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::NOT_FOUND);

    // ws B cannot revoke ws A's key
    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/apikeys/{a_id}/revoke")),
            &admin_b,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), axum::http::StatusCode::NOT_FOUND);

    // forging the bearer's ws field to B (a record that lives in ws A) is refused — the store
    // namespace wall means the id is simply absent in ws B.
    let forged = format!(
        "lbk_ws-b.{}.{}",
        a_bearer
            .strip_prefix("lbk_ws-a.")
            .unwrap()
            .split('.')
            .next()
            .unwrap(),
        a_bearer.split('.').nth(2).unwrap()
    );
    let r = app
        .clone()
        .oneshot(bearer(mcp_call("outbox.status", json!({})), &forged))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        axum::http::StatusCode::UNAUTHORIZED,
        "forged-ws bearer refused"
    );
}
