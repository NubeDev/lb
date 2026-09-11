//! The **contractor's public routes**, over the REAL router (case-plane scope §"Gateway — the token
//! principal", wave 2).
//!
//! Everything here is real: a booted `Node`, the real `router()`, the real caps wall, the real
//! outbox relay. The one permitted external is the email provider — and it is what makes these
//! tests honest, because the token they present is read out of the mail that was actually sent. The
//! store keeps only a hash, so there is no other way to get one, which is exactly the property
//! under test.
//!
//! What it pins:
//!   - `GET /r/{token}` is a PAGE, not a data route — it must not answer JSON;
//!   - the three `/public/case/request*` routes work with no session at all;
//!   - a withdrawn or expired token is **410** with a body that never says which;
//!   - an unknown token is **404**, and never a login prompt;
//!   - the routes are per-IP rate limited (the same fixed-window limiter the invite route uses).
//!
//! **This half:** the refusals — a dead link, an unknown one, a withdrawn one, and the per-IP rate limit.

mod common;

use std::sync::Arc;

use axum::http::StatusCode;
use lb_host::{relay_outbox, EmailTarget, RecordingEmailProvider};
use lb_role_gateway::router;
use serde_json::json;
use tower::ServiceExt;

use common::case_request::*;
// Specific to the rate-limit case, so imported here rather than re-exported to every consumer.
use lb_role_gateway::INVITE_ACCEPT_MAX_PER_WINDOW;

/// A withdrawn ask and an unknown token: 410 and 404, and the 410 body never commits to WHY.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dead_link_is_410_and_an_unknown_one_is_404() {
    let (app, _id, token) = rig().await;

    // Unknown, but well-formed: 404, and never a login prompt.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbr_nube.neverminted",
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    let body = body_of(res).await.to_lowercase();
    assert!(
        !body.contains("password") && !body.contains("sign in") && !body.contains("log in"),
        "a dead request link must never offer a login: {body}"
    );

    // Not a request token at all.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbi_an_invite",
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    // The live token still resolves — so the two refusals above are about the TOKEN, not the route.
    let res = app
        .clone()
        .oneshot(get(
            &format!("/public/case/request?token={token}"),
            "203.0.113.22",
        ))
        .await
        .unwrap();
    assert_eq!(
        res.status(),
        StatusCode::OK,
        "still live before the withdraw"
    );
}

/// The 410 disjunction, driven through a real withdraw on the same node.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_withdrawn_link_is_410_with_a_body_that_never_says_which() {
    let (gw, key) = common::gateway().await;
    let session = common::bootstrap::provision_admin(&gw, "user:test", WS).await;
    let node = gw.node.clone();
    let principal = lb_auth::verify(&key, &session, common::NOW).expect("session");

    let raised = tool(
        &node,
        &principal,
        "insight.raise",
        json!({
            "dedup_key": "gw-withdraw",
            "severity": "warning",
            "title": "probe",
            "origin": { "kind": "rule", "ref": "rule:probe" },
            "ts": NOW_MS,
        }),
    )
    .await;
    let case_id = tool(
        &node,
        &principal,
        "insight.get",
        json!({ "id": raised["id"] }),
    )
    .await["case_id"]
        .as_str()
        .unwrap()
        .to_string();
    tool(
        &node,
        &principal,
        "party.upsert",
        json!({
            "id": "northern",
            "kind": "contractor",
            "name": "Northern",
            "contact": { "email": "ops@northern.example" },
            "default_ask_window_h": 24,
        }),
    )
    .await;
    let request = tool(
        &node,
        &principal,
        "case.request.send",
        json!({
            "case_id": case_id, "party_id": "northern", "ask": "attend", "ts": NOW_MS,
        }),
    )
    .await;

    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), node.store.clone());
    relay_outbox(&node.store, WS, &target, common::NOW)
        .await
        .unwrap();
    let raw = token_from(&provider.sends()[0].body);

    tool(
        &node,
        &principal,
        "case.request.withdraw",
        json!({
            "id": request["id"], "ts": NOW_MS + 1,
        }),
    )
    .await;

    let app = router(gw);
    let res = app
        .oneshot(get(
            &format!("/public/case/request?token={raw}"),
            "203.0.113.23",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::GONE);
    let body = body_of(res).await.to_lowercase();
    // The disjunction, in full. Naming one of the three tells a prober the token was real.
    for word in ["completed", "withdrawn", "timed out"] {
        assert!(
            body.contains(word),
            "the 410 body must stay a disjunction: {body}"
        );
    }
}

/// The routes are rate limited per client IP — they hash a presented secret and answer differently
/// per outcome, which is the same token-oracle posture that put a limiter on the invite route.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_public_case_request_routes_are_rate_limited_per_ip() {
    let (app, _id, _token) = rig().await;
    let ip = "203.0.113.24";

    // Burst with a bogus token: every hit that reaches the handler is a 404, and the (MAX+1)-th is
    // refused before it. The window is wall-clock, so allow one retry round (a broken limiter fails
    // both).
    let mut limited = false;
    for _round in 0..2 {
        for _ in 0..=INVITE_ACCEPT_MAX_PER_WINDOW {
            let res = app
                .clone()
                .oneshot(get("/public/case/request?token=lbr_nube.bogus", ip))
                .await
                .unwrap();
            if res.status() == StatusCode::TOO_MANY_REQUESTS {
                limited = true;
                break;
            }
            assert_eq!(res.status(), StatusCode::NOT_FOUND);
        }
        if limited {
            break;
        }
    }
    assert!(limited, "the ceiling must reject past its limit");

    // A different client is untouched.
    let res = app
        .clone()
        .oneshot(get(
            "/public/case/request?token=lbr_nube.bogus",
            "198.51.100.30",
        ))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
