//! Embedder-credential-mode scope — proves `BootConfig::credential_mode` reaches the gateway the
//! embed seam (`boot_full`) builds, so an embedded node can enforce REAL passwords. Before this,
//! `builder.rs` hardwired the password-less check and an embedded login accepted any secret (verified
//! live on a cc-app node: `secret:"WRONG"` → `200`). The door under test is `POST /auth/login
//! {email, password}` — the ONLY human door, since the legacy `POST /login {user, workspace, secret}`
//! was deleted in the pre-production legacy sweep. No mocks (CLAUDE §9 / testing §0): a real
//! `boot_full` node, the real gateway `router`, the real SurrealDB (`mem://`), real argon2 — driven
//! through the same `router().oneshot()` tower seam the gateway crate's route tests use (no port).
//!
//! Boots with the gateway ON (`GatewayMode::Addr` on a loopback port we never actually serve — we
//! drive the `Gateway` value `RunningNode` hands back). `hello_demo`/`reactors` OFF keep the boot to
//! the store+auth+MCP+gateway subset the assertion needs. The FIRST admin comes from the boot seed
//! (`seed_user` + `seed_credential` + `seed_email`) — the blessed provisioning path, and the only
//! bootstrap there is now that first-login-into-an-empty-workspace is gone.

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

/// Boot an embedded node with the gateway ON, the given credential mode, and a seeded first admin
/// (`user:test` / `test@nube-io.com` / `pw`). Returns the `Gateway` `boot_full` built — the one whose
/// credential check the `credential_mode` field selected.
async fn boot_gateway(mode: CredentialMode, admin_password: Option<&str>) -> Gateway {
    let mut cfg = BootConfig::default();
    cfg.reactors = false;
    cfg.hello_demo = false;
    // A loopback address the ritual builds the gateway on. We drive the returned `Gateway` value via
    // its `router` (a tower service) rather than serving HTTP — the address is never actually bound.
    cfg.gateway = GatewayMode::Addr("127.0.0.1:0".parse().unwrap());
    cfg.credential_mode = mode;
    cfg.seed_user = Some(ADMIN_SUB.into());
    cfg.seed_email = Some(ADMIN_EMAIL.into());
    cfg.seed_credential = admin_password.map(str::to_string);
    let running: RunningNode = boot_full(cfg).await.expect("embedded boot");
    running.gateway.expect("gateway is on (Addr mode)").0
}

/// The seeded first admin (the boot seed's defaults + the email/password this suite pins).
const ADMIN_SUB: &str = "user:test";
const ADMIN_EMAIL: &str = "test@nube-io.com";
const ADMIN_PASSWORD: &str = "dev-admin-pw";

/// `POST /auth/login {email, password}` — returns the whole reply so a test can assert the status
/// AND read the token out of the 1-workspace auto-skip branch.
async fn auth_login(gw: &Gateway, email: &str, password: &str) -> (StatusCode, Value) {
    let resp = router(gw.clone())
        .oneshot(json_post(
            "/auth/login",
            json!({ "email": email, "password": password }),
        ))
        .await
        .unwrap();
    let status = resp.status();
    if status != StatusCode::OK {
        return (status, Value::Null);
    }
    (status, json_body(resp).await)
}

/// Log in and assert the full-session branch, returning the bearer token.
async fn login_token(gw: &Gateway, email: &str, password: &str) -> String {
    let (status, reply) = auth_login(gw, email, password).await;
    assert_eq!(status, StatusCode::OK, "login {email} expected 200");
    reply["token"]
        .as_str()
        .expect("the 1-workspace branch mints a full token")
        .to_string()
}

/// The status of a login attempt (for the deny cases).
async fn login_status(gw: &Gateway, email: &str, password: &str) -> StatusCode {
    auth_login(gw, email, password).await.0
}

/// THE HEADLINE: a `boot_full` node built with `credential_mode: PasswordHash` enforces the argon2
/// credential over its real `/auth/login` — wrong/absent password `401`s, the right one `200`s. This
/// is the exact behaviour an embedded node could NOT get before the field existed (login accepted any
/// secret). The person under test is provisioned by the seeded admin through the real mediated admin
/// routes (identity + email + global password + membership).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_password_hash_mode_enforces_the_credential() {
    let gw = boot_gateway(CredentialMode::PasswordHash, Some(ADMIN_PASSWORD)).await;
    // The seeded admin signs in with the seeded password — the bootstrap paradox fix (no admin token
    // is needed to seed the FIRST admin's credential).
    let admin = login_token(&gw, ADMIN_EMAIL, ADMIN_PASSWORD).await;

    // The admin provisions bob: global identity + email, global password, membership. All mediated,
    // all `mcp:identity.manage:call` / `mcp:members.manage:call` gated server-side.
    for (uri, body) in [
        (
            "/admin/identities".to_string(),
            json!({ "sub": "user:bob", "email": "bob@nube.com" }),
        ),
        (
            "/admin/identities/user:bob/password".to_string(),
            json!({ "secret": "hunter2" }),
        ),
        ("/admin/members".to_string(), json!({ "sub": "user:bob" })),
    ] {
        let resp = router(gw.clone())
            .oneshot(bearer(json_post(&uri, body), &admin))
            .await
            .unwrap();
        assert!(
            resp.status().is_success(),
            "admin provisioning {uri} → {}",
            resp.status()
        );
    }

    // Now the PasswordHash gateway (built by `boot_full` from the field) enforces it:
    // right password → 200 + token; wrong → 401; absent → 401.
    let token = login_token(&gw, "bob@nube.com", "hunter2").await;
    assert!(!token.is_empty(), "right password mints a token");

    assert_eq!(
        login_status(&gw, "bob@nube.com", "WRONG").await,
        StatusCode::UNAUTHORIZED,
        "wrong password → 401 (this was 200 on an embedded node before credential_mode)"
    );
    assert_eq!(
        login_status(&gw, "bob@nube.com", "").await,
        StatusCode::UNAUTHORIZED,
        "absent password → 401"
    );
}

/// BACK-COMPAT: a `boot_full` node with the default (`DevTrustAny`) still password-less-`200`s, so
/// no existing embedder or `boot_full`-based test breaks. `BootConfig::default()` carries
/// `credential_mode: DevTrustAny`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_default_mode_is_password_less() {
    // Default-constructed config → DevTrustAny; a login with any (or no) password 200s.
    let gw = boot_gateway(CredentialMode::DevTrustAny, None).await;
    let token = login_token(&gw, ADMIN_EMAIL, "anything").await;
    assert!(
        !token.is_empty(),
        "DevTrustAny mints a token with any password"
    );
    assert_eq!(
        login_status(&gw, ADMIN_EMAIL, "").await,
        StatusCode::OK,
        "DevTrustAny 200s an empty password too (today's embed behaviour, unchanged)"
    );
}

/// BOOTSTRAP: a `PasswordHash` node with `seed_user` + `seed_credential` + `seed_email` seeds the dev
/// admin's argon2 GLOBAL credential at boot, so that admin can sign in with the seeded password (the
/// bootstrap paradox fix — no admin token is needed to seed the FIRST admin's credential). Wrong
/// password still `401`s. This is the ONLY bootstrap: the deleted `POST /login` used to promote the
/// first caller into an empty workspace, which is exactly the self-promotion hazard the sweep closed.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_full_seeds_the_dev_admin_credential_for_password_hash() {
    let gw = boot_gateway(CredentialMode::PasswordHash, Some(ADMIN_PASSWORD)).await;
    let token = login_token(&gw, ADMIN_EMAIL, ADMIN_PASSWORD).await;
    assert!(
        !token.is_empty(),
        "seeded admin logs in with the seeded password"
    );
    // Wrong password still 401s — the seed sets a REAL argon2 credential, not a bypass.
    assert_eq!(
        login_status(&gw, ADMIN_EMAIL, "WRONG").await,
        StatusCode::UNAUTHORIZED,
        "wrong password 401s even for the seeded admin"
    );
    // And an unknown email is the SAME uniform 401 — no account-enumeration oracle.
    assert_eq!(
        login_status(&gw, "nobody@nube.com", ADMIN_PASSWORD).await,
        StatusCode::UNAUTHORIZED,
        "unknown email → the same 401 as a wrong password"
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
