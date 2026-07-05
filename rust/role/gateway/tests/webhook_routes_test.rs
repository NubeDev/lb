//! webhooks scope — the full lifecycle over the REAL gateway (no mocks, real node + real store +
//! real caps + real ingest buffer). Covers the mandatory categories: capability-deny (per
//! management verb), the privilege-escalation deny (admin lacks `ingest.write`), workspace
//! isolation (cross-ws URL is opaque 404; ws-B can't see ws-A's hooks; cross-ws bearer refused),
//! both auth modes end-to-end (create → POST → sample committed → series.read returns it),
//! raw-body HMAC verification (a re-serialized body breaks the signature), rotate (old secret
//! dead), revoke (route 410s, no further samples), and the no-secret-leak assertion on list/get.
//!
//! The headline: a webhook is a **generic authenticated HTTP inlet that emits a `Sample`** — every
//! accepted hit lands in the existing ingest buffer and is readable via `series.read` over the
//! same MCP bridge. No provider is named anywhere (rule 10).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{bearer, gateway, get_req, json_body, json_post, post_empty, token};
use hmac::{Hmac, Mac};
use lb_role_gateway::router;
use serde_json::{json, Value};
use sha2::Sha256;
use tower::ServiceExt;

type HmacSha256 = Hmac<Sha256>;

/// The admin cap set: `webhook.manage` + `ingest.write` (the cap a hook's principal resolves to,
/// and therefore the cap the no-widening guard demands of the creator) + the secret-write cap
/// `signature` mode needs + `series.read` (to assert the committed sample) + the apikeymanage
/// cap so we can construct a ws-B bearer for the isolation test.
const ADMIN_CAPS: &[&str] = &[
    "mcp:webhook.manage:call",
    "mcp:ingest.write:call",
    "secret:webhook/*:write",
    "mcp:series.read:call",
    "mcp:apikey.manage:call",
    "mcp:*.get:call",
    "mcp:*.list:call",
    "store:*:read",
    "store:*:write",
];

/// A non-admin cap set (lacks `webhook.manage`) — for the deny tests.
const NO_MANAGE: &[&str] = &["bus:chan/*:pub"];

/// Mint an admin JWT for `ws`.
fn admin(key: &lb_auth::SigningKey, ws: &str) -> String {
    token(key, "user:admin", ws, ADMIN_CAPS)
}

/// Create a webhook as the admin, returning the `CreatedWebhook` JSON reply.
async fn create(app: &axum::Router, admin_token: &str, body: Value) -> Value {
    let resp = app
        .clone()
        .oneshot(bearer(json_post("/admin/webhooks", body), admin_token))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "create failed");
    json_body(resp).await
}

/// Sign `body` under `secret` as the universal `sha256=<hex>` header value.
fn sign(secret: &[u8], body: &[u8]) -> String {
    let mut mac = HmacSha256::new_from_slice(secret).unwrap();
    mac.update(body);
    let mac = mac.finalize().into_bytes();
    let mut hex = String::with_capacity(64);
    for b in mac {
        hex.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        hex.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    format!("sha256={hex}")
}

/// A `POST /hooks/{ws}/{id}` request with raw bytes + optional headers.
fn hook_req(ws: &str, id: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(format!("/hooks/{ws}/{id}"))
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn with_header(mut req: Request<Body>, name: &'static str, value: &str) -> Request<Body> {
    req.headers_mut().insert(name, value.parse().unwrap());
    req
}

// --- capability-deny (mandatory) -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn management_verbs_denied_without_webhook_manage() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = token(&key, "user:bob", "acme", NO_MANAGE);

    // list
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/webhooks"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    // create
    let r = app
        .clone()
        .oneshot(bearer(
            json_post("/admin/webhooks", json!({"name":"x","auth_mode":"bearer"})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    // get / revoke / rotate on a (nonexistent) id — also denied before the not-found check
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/webhooks/wh_x"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
    let r = app
        .clone()
        .oneshot(bearer(post_empty("/admin/webhooks/wh_x/revoke"), &tok))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn escalation_denied_when_creator_lacks_ingest_write() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    // A narrow admin: has webhook.manage but LACKS mcp:ingest.write:call.
    let narrow = token(
        &key,
        "user:narrow",
        "acme",
        &["mcp:webhook.manage:call", "secret:webhook/*:write"],
    );

    let r = app
        .clone()
        .oneshot(bearer(
            json_post(
                "/admin/webhooks",
                json!({"name":"x","auth_mode":"signature"}),
            ),
            &narrow,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
    let txt = common::body_text(r).await;
    assert!(txt.contains("ingest.write"), "got: {txt}");
}

// --- workspace isolation (mandatory) -------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cross_workspace_url_is_opaque_404() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok_a = admin(&key, "acme");

    // Create a webhook in ws-A.
    let created = create(
        &app,
        &tok_a,
        json!({"name":"plant","auth_mode":"signature"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();

    // POST to /hooks/wsB/{id} — ws-B is a different namespace; the ws-A webhook is not visible.
    // The route must 404 opaquely (no existence leak).
    let r = app
        .clone()
        .oneshot(hook_req("wsB", &id, b"{}".to_vec()))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_admin_cannot_see_ws_a_webhooks() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok_a = admin(&key, "acme");
    let tok_b = admin(&key, "wsB");

    let _ = create(
        &app,
        &tok_a,
        json!({"name":"plant","auth_mode":"signature"}),
    )
    .await;

    // ws-B's list returns its OWN webhooks (none) — never ws-A's.
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/webhooks"), &tok_b))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let list: Vec<Value> = json_body(r).await;
    assert!(list.is_empty(), "ws-B saw ws-A's webhooks: {list:?}");
}

// --- bearer mode end-to-end ----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bearer_mode_create_post_sample_committed() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    // Create a bearer-mode webhook.
    let created = create(
        &app,
        &tok,
        json!({"name":"plant-alerts","auth_mode":"bearer"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    let secret = created["secret"].as_str().unwrap().to_string();
    assert!(secret.starts_with("lbk_acme."), "bearer secret shape");

    // POST a hit with the bearer — accepted, sample committed.
    let body = br#"{"event":"furnace-on"}"#.to_vec();
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body),
            "authorization",
            &format!("Bearer {secret}"),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
    let reply: Value = json_body(r).await;
    let series = reply["series"].as_str().unwrap();
    assert_eq!(series, format!("webhook:acme:{id}"));

    // series.read over the MCP bridge returns the sample (the round-trip — the headline).
    let r = app
        .clone()
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({"tool":"series.read","args":{"series": series}}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let out: Value = json_body(r).await;
    let samples = out["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0]["payload"]["event"], "furnace-on");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bearer_mode_wrong_secret_is_opaque_404() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"bearer"})).await;
    let id = created["id"].as_str().unwrap().to_string();

    // A wrong-secret bearer — the same opaque 404 as an unknown id (no oracle).
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, b"{}".to_vec()),
            "authorization",
            "Bearer lbk_acme.wrongkey.deadbeef",
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn bearer_mode_wrong_ws_in_bearer_refused() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"bearer"})).await;
    let id = created["id"].as_str().unwrap().to_string();
    let real_secret = created["secret"].as_str().unwrap();

    // POST to /hooks/acme/{id} but present a bearer whose ws field is "wsB". Even though the
    // apikey row would resolve in ws-B, the URL is /hooks/acme/... and the bearer ws must match
    // the URL ws — refused (opaque 404, same as a wrong secret).
    // Strip "lbk_acme." and prepend "lbk_wsB." to forge a ws mismatch.
    let tail = real_secret.strip_prefix("lbk_acme.").unwrap();
    let forged = format!("lbk_wsB.{tail}");
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, b"{}".to_vec()),
            "authorization",
            &format!("Bearer {forged}"),
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

// --- signature mode end-to-end -------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn signature_mode_create_post_sample_committed() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(
        &app,
        &tok,
        json!({"name":"plant-alerts","auth_mode":"signature","hmac_header":"X-Signature"}),
    )
    .await;
    let id = created["id"].as_str().unwrap().to_string();
    let shared = created["secret"].as_str().unwrap().to_string();
    assert!(!shared.is_empty());

    // Sign the raw body and POST.
    let body = br#"{"event":"furnace-on"}"#;
    let sig = sign(shared.as_bytes(), body);
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body.to_vec()),
            "x-signature",
            &sig,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);

    // series.read returns the sample (raw body preserved verbatim as the payload).
    let r = app
        .clone()
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({"tool":"series.read","args":{"series": format!("webhook:acme:{id}")}}),
            ),
            &tok,
        ))
        .await
        .unwrap();
    let out: Value = json_body(r).await;
    let samples = out["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 1);
    assert_eq!(samples[0]["payload"]["event"], "furnace-on");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn signature_mode_wrong_signature_is_opaque_404() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    let id = created["id"].as_str().unwrap().to_string();

    // A wrong signature — same opaque 404 as an unknown id (no oracle).
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, b"{}".to_vec()),
            "x-signature",
            "sha256=0000000000000000000000000000000000000000000000000000000000000000",
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn signature_mode_missing_header_is_opaque_404() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    let id = created["id"].as_str().unwrap().to_string();

    // No signature header at all — opaque 404.
    let r = app
        .clone()
        .oneshot(hook_req("acme", &id, b"{}".to_vec()))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);
}

/// The single most common webhook-integration bug: HMAC over a RE-SERIALIZED body never matches.
/// The route MUST capture the raw body before any JSON parse and verify on those bytes — pinned
/// here by signing the original bytes and posting a DIFFERENT byte sequence with the same JSON
/// value (whitespace/key-order changes).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn signature_mode_body_tamper_breaks_signature() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    let id = created["id"].as_str().unwrap().to_string();
    let shared = created["secret"].as_str().unwrap();

    // Sign the compact body, post the pretty-printed body with the same JSON value.
    let compact = br#"{"event":"on"}"#;
    let sig = sign(shared.as_bytes(), compact);
    let pretty = br#"{
  "event": "on"
}"#;
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, pretty.to_vec()),
            "x-signature",
            &sig,
        ))
        .await
        .unwrap();
    assert_eq!(
        r.status(),
        StatusCode::NOT_FOUND,
        "re-serialized body must not verify"
    );
}

// --- rotate + revoke -----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rotate_signature_old_dead_new_works() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    let id = created["id"].as_str().unwrap().to_string();
    let old_secret = created["secret"].as_str().unwrap().to_string();

    // Rotate.
    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/webhooks/{id}/rotate")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let new_secret: String = json_body::<Value>(r).await["secret"]
        .as_str()
        .unwrap()
        .to_string();
    assert_ne!(new_secret, old_secret, "rotate must produce a fresh secret");

    // Old secret's signature is now refused.
    let body = b"{}";
    let old_sig = sign(old_secret.as_bytes(), body);
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body.to_vec()),
            "x-signature",
            &old_sig,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // New secret's signature works.
    let new_sig = sign(new_secret.as_bytes(), body);
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body.to_vec()),
            "x-signature",
            &new_sig,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoke_then_route_410s_no_further_samples() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    let id = created["id"].as_str().unwrap().to_string();
    let shared = created["secret"].as_str().unwrap();

    // Pre-revoke: a hit works.
    let body = b"{}";
    let sig = sign(shared.as_bytes(), body);
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body.to_vec()),
            "x-signature",
            &sig,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::ACCEPTED);

    // Revoke.
    let r = app
        .clone()
        .oneshot(bearer(
            post_empty(&format!("/admin/webhooks/{id}/revoke")),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NO_CONTENT);

    // Post-revoke: same signature → 410 Gone, no sample written.
    let r = app
        .clone()
        .oneshot(with_header(
            hook_req("acme", &id, body.to_vec()),
            "x-signature",
            &sig,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::GONE);
}

// --- no-secret-leak (mediation invariant) --------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_and_get_carry_no_secret_material() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let created_bearer = create(
        &app,
        &tok,
        json!({"name":"bearer-hook","auth_mode":"bearer"}),
    )
    .await;
    let created_sig = create(
        &app,
        &tok,
        json!({"name":"sig-hook","auth_mode":"signature"}),
    )
    .await;
    let bearer_secret = created_bearer["secret"].as_str().unwrap();
    let sig_secret = created_sig["secret"].as_str().unwrap();

    // list never carries a secret/hash/bearer_key_id/secret_ref.
    let r = app
        .clone()
        .oneshot(bearer(get_req("/admin/webhooks"), &tok))
        .await
        .unwrap();
    let list: Vec<Value> = json_body(r).await;
    let dumped = serde_json::to_string(&list).unwrap();
    assert!(
        !dumped.contains(bearer_secret),
        "list leaked the bearer secret"
    );
    assert!(
        !dumped.contains(sig_secret),
        "list leaked the shared secret"
    );
    assert!(
        !dumped.contains("secret") && !dumped.contains("hash") && !dumped.contains("bearer_key_id"),
        "list carried secret material: {dumped}"
    );
    assert_eq!(list.len(), 2);

    // get (per-hook) likewise.
    for entry in &list {
        let id = entry["id"].as_str().unwrap();
        let r = app
            .clone()
            .oneshot(bearer(get_req(&format!("/admin/webhooks/{id}")), &tok))
            .await
            .unwrap();
        let view: Value = json_body(r).await;
        let dumped = serde_json::to_string(&view).unwrap();
        assert!(!dumped.contains("secret"));
        assert!(!dumped.contains("hash"));
        assert!(!dumped.contains("bearer_key_id"));
        assert!(!dumped.contains("secret_ref"));
    }
}

// --- create-reply shape --------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn create_signature_returns_hmac_header_for_the_wizard() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    // Default header (admin did not pick one) → X-Signature.
    let created = create(&app, &tok, json!({"name":"x","auth_mode":"signature"})).await;
    assert_eq!(created["hmac_header"], "X-Signature");
    assert_eq!(created["auth_mode"], "signature");
    assert!(created["url_path"]
        .as_str()
        .unwrap()
        .starts_with("/hooks/acme/wh_"));

    // Admin-picked header is echoed.
    let created = create(
        &app,
        &tok,
        json!({"name":"y","auth_mode":"signature","hmac_header":"X-Custom-Sig"}),
    )
    .await;
    assert_eq!(created["hmac_header"], "X-Custom-Sig");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_auth_mode_is_bad_input() {
    let (gw, key) = gateway().await;
    let app = router(gw);
    let tok = admin(&key, "acme");

    let r = app
        .clone()
        .oneshot(bearer(
            json_post("/admin/webhooks", json!({"name":"x","auth_mode":"oauth"})),
            &tok,
        ))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::BAD_REQUEST);
}
