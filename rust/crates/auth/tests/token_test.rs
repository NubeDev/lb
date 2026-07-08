//! Token mint→verify round-trip with an INJECTED clock (testing §3: never wall-clock).
//! Proves the §13 token shape signs, verifies offline, and rejects tamper/expiry.

use lb_auth::{mint, verify, AuthError, Claims, Role, SigningKey};

fn claims(exp: u64) -> Claims {
    Claims {
        sub: "user:ada".into(),
        ws: "acme".into(),
        role: Role::WorkspaceAdmin,
        caps: vec!["mcp:hello.echo:call".into()],
        iat: 0,
        exp,
        constraint: None,
        run_id: None,
    }
}

#[test]
fn round_trips_and_carries_the_workspace_claim() {
    let key = SigningKey::generate();
    let token = mint(&key, &claims(100));
    let p = verify(&key, &token, 1).expect("valid token verifies");
    assert_eq!(p.ws(), "acme");
    assert_eq!(p.sub(), "user:ada");
    assert_eq!(p.role(), Role::WorkspaceAdmin);
    assert_eq!(p.caps(), &["mcp:hello.echo:call".to_string()]);
}

#[test]
fn rejects_expired_token_against_injected_now() {
    let key = SigningKey::generate();
    let token = mint(&key, &claims(50));
    // now == exp is expired (>= exp); use a clearly-past time.
    assert_eq!(verify(&key, &token, 50), Err(AuthError::Expired));
    assert_eq!(verify(&key, &token, 999), Err(AuthError::Expired));
}

#[test]
fn rejects_token_signed_by_a_different_key() {
    let issuer = SigningKey::generate();
    let attacker = SigningKey::generate();
    let token = mint(&issuer, &claims(100));
    // Verifying against the wrong public key must fail the signature check.
    assert_eq!(verify(&attacker, &token, 1), Err(AuthError::BadToken));
}

#[test]
fn rejects_tampered_token() {
    let key = SigningKey::generate();
    let mut token = mint(&key, &claims(100));
    // Flip a character in the payload section.
    let mid = token.len() / 2;
    let bytes = unsafe { token.as_bytes_mut() };
    bytes[mid] = if bytes[mid] == b'A' { b'B' } else { b'A' };
    assert_eq!(verify(&key, &token, 1), Err(AuthError::BadToken));
}
