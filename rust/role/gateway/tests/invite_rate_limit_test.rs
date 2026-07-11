//! The public invite route's per-IP rate limit (invites scope: "the public route ships
//! rate-limited from day one"). Drives the REAL router — `POST /public/invite/accept` through the
//! `invite_accept_rate_limit` middleware — and asserts the (MAX+1)-th hit from one client is 429
//! while a different client is untouched. The limiter itself is unit-tested in
//! `routes/rate_limit.rs`; this proves the route wiring.
//!
//! Each test uses its own distinct `x-forwarded-for` keys because the limiter is process-wide
//! (the wall clock places all hits in one window at test speed — MAX is per key, so keys isolate).

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use lb_role_gateway::{router, INVITE_ACCEPT_MAX_PER_WINDOW};
use tower::ServiceExt;

fn accept_req(ip: &str) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/public/invite/accept")
        .header("content-type", "application/json")
        .header("x-forwarded-for", ip)
        .body(Body::from(
            r#"{"token":"lbi_nope","workspace":"acme","secret":"pw"}"#,
        ))
        .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn public_invite_accept_is_rate_limited_per_ip() {
    let (gw, _key) = common::gateway().await;
    let app = router(gw);

    // Up to MAX hits pass the limiter (they 404 on the bogus token — the handler ran), then the
    // (MAX+1)-th from the SAME client is refused. The window is wall-clock; if it happens to roll
    // mid-burst the extra hit lands in a fresh window, so allow ONE retry round (never flaky, and
    // a broken limiter still fails both rounds).
    let mut limited = false;
    for _round in 0..2 {
        for i in 0..INVITE_ACCEPT_MAX_PER_WINDOW {
            let res = app
                .clone()
                .oneshot(accept_req("203.0.113.7"))
                .await
                .unwrap();
            if res.status() == StatusCode::TOO_MANY_REQUESTS {
                // Prior round's hits still count in this window — the limit already engaged.
                limited = true;
                break;
            }
            assert_eq!(
                res.status(),
                StatusCode::NOT_FOUND,
                "hit {i} must reach the handler (bogus token → 404), not be limited"
            );
        }
        if limited {
            break;
        }
        let res = app
            .clone()
            .oneshot(accept_req("203.0.113.7"))
            .await
            .unwrap();
        if res.status() == StatusCode::TOO_MANY_REQUESTS {
            limited = true;
            break;
        }
    }
    assert!(limited, "the (MAX+1)-th hit from one client must be 429");

    // A DIFFERENT client is not punished.
    let res = app
        .clone()
        .oneshot(accept_req("198.51.100.9"))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}
