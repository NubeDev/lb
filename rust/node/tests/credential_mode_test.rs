//! Embedder-credential-mode scope — proves `BootConfig::credential_mode` reaches the gateway the
//! embed seam (`boot_full`) builds, so an embedded node can enforce REAL passwords. Before this,
//! `builder.rs` hardwired `DevTrustAny` and an embedded `POST /login` accepted any secret (verified
//! live on a cc-app node: `secret:"WRONG"` → `200`). No mocks (CLAUDE §9 / testing §0): a real
//! `boot_full` node, the real gateway `router`, the real SurrealDB (`mem://`), real argon2 — driven
//! through the same `router().oneshot()` tower seam the gateway crate's route tests use (no port).
//!
//! Boots with the gateway ON (`GatewayMode::Addr` on a loopback port we never actually serve — we
//! drive the `Gateway` value `RunningNode` hands back). `hello_demo`/`reactors` OFF and `seed_user`
//! `None` keep the boot to the store+auth+MCP+gateway subset the assertion needs.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use lb_node::{boot_full, BootConfig, CredentialMode, GatewayMode, RunningNode};
use lb_role_gateway::{router, Gateway};
use serde_json::{json, Value};
use tower::ServiceExt;

/// A `POST` request with a JSON body to `uri`.
fn json_post(uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

/// Attach a bearer token to a request.
fn bearer(req: Request<Body>, token: &str) -> Request<Body> {
    let (mut parts, body) = req.into_parts();
    parts
        .headers
        .insert("authorization", format!("Bearer {token}").parse().unwrap());
    Request::from_parts(parts, body)
}

/// Deserialize a response body as a JSON value.
async fn json_body(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Boot an embedded node with the gateway ON and the given credential mode. Returns the `Gateway`
/// `boot_full` built (the one whose login check the field selected) AND a sibling `DevTrustAny`
/// gateway over the SAME node/key — the sibling is the password-less admin path a test uses to seed
/// a credential before asserting the real gateway enforces it (both share one store: one node).
/// `hello_demo`/`reactors` OFF, no dev seed — the minimal gateway-serving subset.
async fn boot_gateways(mode: CredentialMode) -> (Gateway, Gateway) {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.reactors = false;
    cfg.hello_demo = false;
    // A loopback address the ritual builds the gateway on. We drive the returned `Gateway` value via
    // its `router` (a tower service) rather than serving HTTP — the address is never actually bound.
    cfg.gateway = GatewayMode::Addr("127.0.0.1:0".parse().unwrap());
    cfg.credential_mode = mode;
    // Keep the key so the sibling DevTrustAny gateway signs/verifies with the SAME identity the
    // node installed (a mismatched key would 401 every sibling-minted token).
    let key = cfg.signing_key.clone();
    let running: RunningNode = boot_full(cfg).await.expect("embedded boot");
    let node = running.node.clone();
    let target = running.gateway.expect("gateway is on (Addr mode)").0;
    // The sibling always DevTrustAny (password-less), for seeding — over the same node.
    let seeder = Gateway::new_live(node, key);
    (target, seeder)
}

/// Log in over the real `/login` route, asserting `200`, and return the bearer token.
async fn login(gw: &Gateway, user: &str, ws: &str, secret: &str) -> String {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws, "secret": secret }),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "login {user}@{ws} expected 200");
    let reply = json_body(resp).await;
    reply["token"].as_str().unwrap().to_string()
}

/// The status of a `/login` attempt (no token assertion — for the deny cases).
async fn login_status(gw: &Gateway, user: &str, ws: &str, secret: &str) -> StatusCode {
    router(gw.clone())
        .oneshot(json_post(
            "/login",
            json!({ "user": user, "workspace": ws, "secret": secret }),
        ))
        .await
        .unwrap()
        .status()
}

/// THE HEADLINE: a `boot_full` node built with `credential_mode: PasswordHash` enforces the argon2
/// credential over its real `/login` — wrong/absent secret `401`s, the right secret `200`s. This is
/// the exact behaviour an embedded node could NOT get before the field existed (login accepted any
/// secret). The credential is set through the real mediated admin verb (`identity.set_credential`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_password_hash_mode_enforces_the_credential() {
    // `gw` is the PasswordHash gateway `boot_full` built from the field; `dev_gw` is a password-less
    // sibling over the SAME node, used only to bootstrap an admin + seed bob's credential (a
    // PasswordHash gateway can't bootstrap — alice has no credential yet, so it would 401 her).
    let (gw, dev_gw) = boot_gateways(CredentialMode::PasswordHash).await;

    // First login into the empty workspace bootstraps `alice` as workspace-admin (decision #3).
    let admin = login(&dev_gw, "user:alice", "acme", "").await;

    // Admin adds bob as a member and sets his argon2 credential over the real MCP bridge.
    let resp = router(dev_gw.clone())
        .oneshot(bearer(
            json_post("/admin/members", json!({ "sub": "user:bob" })),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT, "admin adds bob");
    let resp = router(dev_gw.clone())
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                json!({ "tool": "identity.set_credential",
                        "args": { "user": "user:bob", "secret": "hunter2" } }),
            ),
            &admin,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "admin sets bob's credential");

    // Now the PasswordHash gateway (built by `boot_full` from the field) enforces it:
    // right secret → 200 + token; wrong secret → 401; absent secret → 401.
    let token = login(&gw, "user:bob", "acme", "hunter2").await;
    assert!(!token.is_empty(), "right password mints a token");

    assert_eq!(
        login_status(&gw, "user:bob", "acme", "WRONG").await,
        StatusCode::UNAUTHORIZED,
        "wrong password → 401 (this was 200 on an embedded node before credential_mode)"
    );
    assert_eq!(
        login_status(&gw, "user:bob", "acme", "").await,
        StatusCode::UNAUTHORIZED,
        "absent password → 401"
    );
}

/// BACK-COMPAT: a `boot_full` node with the default (`DevTrustAny`) still password-less-`200`s, so
/// no existing embedder or `boot_full`-based test breaks. `BootConfig::default()` carries
/// `credential_mode: DevTrustAny`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_default_mode_is_password_less() {
    // Default-constructed config → DevTrustAny; a login with any (or no) secret 200s.
    let (gw, _seeder) = boot_gateways(CredentialMode::DevTrustAny).await;
    let token = login(&gw, "user:ada", "acme", "anything").await;
    assert!(!token.is_empty(), "DevTrustAny mints a token with any secret");
    assert_eq!(
        login_status(&gw, "user:ada", "acme", "").await,
        StatusCode::OK,
        "DevTrustAny 200s an empty secret too (today's embed behaviour, unchanged)"
    );
}

/// BOOTSTRAP: a `PasswordHash` node with `seed_user` + `seed_credential` seeds the dev admin's
/// argon2 credential at boot, so that admin can log in with the seeded password (the bootstrap
/// paradox fix — no admin token is needed to seed the FIRST admin's credential). Wrong secret still
/// `401`s. This is the path an embedder (cc-app) uses so `make seed`'s admin login works under
/// PasswordHash.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_seeds_the_dev_admin_credential_for_password_hash() {
    let mut cfg = BootConfig::default();
    cfg.reactors = false;
    cfg.hello_demo = false;
    cfg.gateway = GatewayMode::Addr("127.0.0.1:0".parse().unwrap());
    cfg.credential_mode = CredentialMode::PasswordHash;
    cfg.seed_user = Some("user:ada".into());
    cfg.seed_credential = Some("dev-admin-pw".into());
    let running = boot_full(cfg).await.expect("embedded boot");
    let gw = running.gateway.expect("gateway on").0;

    // The seeded admin logs in with the seeded password → 200 + token (was impossible: no admin
    // could authenticate to set its own credential under PasswordHash before this).
    let token = login(&gw, "user:ada", "acme", "dev-admin-pw").await;
    assert!(!token.is_empty(), "seeded admin logs in with the seeded password");
    // Wrong password still 401s — the seed sets a REAL argon2 credential, not a bypass.
    assert_eq!(
        login_status(&gw, "user:ada", "acme", "WRONG").await,
        StatusCode::UNAUTHORIZED,
        "wrong password 401s even for the seeded admin"
    );
}

/// `BootConfig::from_env()` mirrors the standalone binary's `LB_DEV_LOGIN` rule so the `node` binary
/// is unchanged: unset ⇒ `PasswordHash`, set/non-empty ⇒ `DevTrustAny`. The `Default` embed default
/// stays `DevTrustAny` regardless (asserted above) — the two constructors differ ON PURPOSE.
#[test]
fn from_env_mirrors_lb_dev_login_but_default_stays_dev_trust_any() {
    // Serialize env mutation within this test (cargo runs test fns concurrently; env is process-global).
    std::env::remove_var("LB_DEV_LOGIN");
    assert_eq!(
        BootConfig::from_env().credential_mode,
        CredentialMode::PasswordHash,
        "LB_DEV_LOGIN unset ⇒ PasswordHash (matches the standalone binary)"
    );
    std::env::set_var("LB_DEV_LOGIN", "1");
    assert_eq!(
        BootConfig::from_env().credential_mode,
        CredentialMode::DevTrustAny,
        "LB_DEV_LOGIN=1 ⇒ DevTrustAny"
    );
    std::env::remove_var("LB_DEV_LOGIN");

    // The embed Default is DevTrustAny regardless of env — the back-compat guarantee.
    assert_eq!(
        BootConfig::default().credential_mode,
        CredentialMode::DevTrustAny,
        "Default::default() is DevTrustAny (back-compat), independent of from_env's binary rule"
    );
}
