//! XOAUTH2 **token refresh** against a real HTTP token endpoint (email-transport scope, Risks: "'support
//! Gmail' is an OAuth problem, not an SMTP problem").
//!
//! An access token lives about an hour, so without refresh "Gmail support" is a config field that
//! breaks an hour after setup. These tests pin the three things that make it real: the grant shape put
//! on the wire, exactly-one-refresh caching, and a failure classification that sends `invalid_grant` to
//! an operator instead of into a five-attempt retry storm.

mod token_endpoint;

use lb_mail::send::auth::{access_token, RefreshRequest, TokenCache};
use token_endpoint::TestTokenEndpoint;

fn request(endpoint: &TestTokenEndpoint) -> RefreshRequest {
    RefreshRequest {
        token_endpoint: endpoint.url(),
        client_id: "1234.apps.googleusercontent.com".into(),
        client_secret: "GOCSPX-clientsecret".into(),
        refresh_token: "1//0gRefreshTokenValue".into(),
    }
}

#[tokio::test]
async fn the_exchange_posts_an_rfc6749_refresh_token_grant() {
    let endpoint = TestTokenEndpoint::minting("ya29.first", 3600).await;
    let client = reqwest::Client::new();

    let token = access_token(&TokenCache::new(), &client, &request(&endpoint))
        .await
        .expect("minted");
    assert_eq!(token, "ya29.first");

    let body = endpoint.bodies().first().cloned().unwrap_or_default();
    assert!(body.contains("grant_type=refresh_token"), "{body}");
    assert!(
        body.contains("refresh_token=1%2F%2F0gRefreshTokenValue"),
        "{body}"
    );
    assert!(body.contains("client_id=1234"), "{body}");
}

#[tokio::test]
async fn a_fresh_token_is_cached_so_a_burst_of_sends_costs_one_refresh() {
    let endpoint = TestTokenEndpoint::minting("ya29.cached", 3600).await;
    let client = reqwest::Client::new();
    let cache = TokenCache::new();
    let req = request(&endpoint);

    for _ in 0..5 {
        assert_eq!(
            access_token(&cache, &client, &req).await.unwrap(),
            "ya29.cached"
        );
    }
    assert_eq!(
        endpoint.hits(),
        1,
        "five sends must share one access token, not mint five"
    );
}

#[tokio::test]
async fn an_expired_token_triggers_exactly_one_more_refresh() {
    // `expires_in: 30` is inside the 60s skew ⇒ the entry is never fresh, which is how an expiring
    // token is exercised without sleeping. The trap it guards is the saturating_sub that could floor
    // the usable window to 0 and then treat the entry as valid forever.
    let endpoint = TestTokenEndpoint::minting("ya29.shortlived", 30).await;
    let client = reqwest::Client::new();
    let cache = TokenCache::new();
    let req = request(&endpoint);

    access_token(&cache, &client, &req).await.unwrap();
    access_token(&cache, &client, &req).await.unwrap();
    assert_eq!(
        endpoint.hits(),
        2,
        "a token past its skew window must be re-minted"
    );
}

#[tokio::test]
async fn rotating_the_sealed_refresh_token_invalidates_the_cached_access_token() {
    let endpoint = TestTokenEndpoint::minting("ya29.forgrantA", 3600).await;
    let client = reqwest::Client::new();
    let cache = TokenCache::new();

    access_token(&cache, &client, &request(&endpoint))
        .await
        .unwrap();
    let rotated = RefreshRequest {
        refresh_token: "1//0gRotatedTokenValue".into(),
        ..request(&endpoint)
    };
    access_token(&cache, &client, &rotated).await.unwrap();
    assert_eq!(
        endpoint.hits(),
        2,
        "an access token minted from the OLD grant must not be reused after rotation"
    );
}

#[tokio::test]
async fn invalid_grant_is_permanent_and_a_5xx_is_retryable() {
    // invalid_grant = the operator's consent was revoked (or the wrong token was sealed). No retry
    // fixes it; the effect must fail with a reason a human can act on.
    let revoked = TestTokenEndpoint::start(
        400,
        r#"{"error":"invalid_grant","error_description":"Token has been expired or revoked."}"#
            .into(),
    )
    .await;
    let client = reqwest::Client::new();
    let err = access_token(&TokenCache::new(), &client, &request(&revoked))
        .await
        .expect_err("a revoked grant must fail");
    assert!(err.is_permanent(), "{err}");
    assert!(err.message().contains("invalid_grant"), "{err}");

    // A provider outage says nothing about our credentials — the outbox should back off and retry.
    let down = TestTokenEndpoint::start(503, r#"{"error":"backend_error"}"#.into()).await;
    let err = access_token(&TokenCache::new(), &client, &request(&down))
        .await
        .expect_err("a 503 must fail this attempt");
    assert!(
        !err.is_permanent(),
        "a provider outage must be retryable: {err}"
    );
}

#[tokio::test]
async fn a_refresh_failure_never_echoes_the_grant_material() {
    // Some providers echo the submitted parameters in their error body. The classifier reads the
    // machine-readable `error` field ONLY, so nothing that could authenticate as us reaches a log.
    let chatty = TestTokenEndpoint::start(
        400,
        r#"{"error":"invalid_client","error_description":"client_secret=GOCSPX-clientsecret refresh_token=1//0gRefreshTokenValue rejected"}"#
            .into(),
    )
    .await;
    let client = reqwest::Client::new();
    let err = access_token(&TokenCache::new(), &client, &request(&chatty))
        .await
        .expect_err("must fail");

    let text = format!("{err} / {err:?}");
    assert!(!text.contains("GOCSPX-clientsecret"), "{text}");
    assert!(!text.contains("1//0gRefreshTokenValue"), "{text}");
    assert!(text.contains("invalid_client"), "{text}");
}
