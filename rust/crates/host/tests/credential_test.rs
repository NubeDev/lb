//! Credential service tests (login-hardening scope) over the REAL store (`mem://`), no mocks:
//! set → verify round-trip, wrong/absent secret, workspace isolation, and the capability-deny gate
//! on `identity.set_credential`.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{credential_verify, identity_set_credential, CredentialCheck};
use lb_store::Store;

/// A principal in `ws` carrying exactly `caps` (a real minted+verified token, like `authz_test`).
fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const MANAGE: &[&str] = &["mcp:identity.manage:call"];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_then_verify_round_trips_and_rejects_a_wrong_or_absent_secret() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", MANAGE);

    // Absent before any set.
    assert_eq!(
        credential_verify(&store, "acme", "user:bob", "hunter2")
            .await
            .unwrap(),
        CredentialCheck::Absent
    );

    identity_set_credential(&store, &admin, "user:bob", "hunter2", 1)
        .await
        .expect("admin sets bob's credential");

    // Right secret → Ok; wrong secret → BadSecret.
    assert_eq!(
        credential_verify(&store, "acme", "user:bob", "hunter2")
            .await
            .unwrap(),
        CredentialCheck::Ok
    );
    assert_eq!(
        credential_verify(&store, "acme", "user:bob", "nope")
            .await
            .unwrap(),
        CredentialCheck::BadSecret
    );
    // A bare handle canonicalizes to the same record.
    assert_eq!(
        credential_verify(&store, "acme", "bob", "hunter2")
            .await
            .unwrap(),
        CredentialCheck::Ok
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_credential_is_workspace_isolated() {
    let store = Store::memory().await.unwrap();
    let admin_acme = principal("user:alice", "acme", MANAGE);

    identity_set_credential(&store, &admin_acme, "user:bob", "hunter2", 1)
        .await
        .unwrap();

    // The password set in acme does not exist in beta (the hard wall §7).
    assert_eq!(
        credential_verify(&store, "beta", "user:bob", "hunter2")
            .await
            .unwrap(),
        CredentialCheck::Absent
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_credential_denies_a_member_and_an_empty_secret() {
    let store = Store::memory().await.unwrap();

    // A member (no identity.manage) is denied — the capability-deny test.
    let member = principal("user:bob", "acme", &["mcp:dashboard.list:call"]);
    assert!(
        identity_set_credential(&store, &member, "user:carol", "x", 1)
            .await
            .is_err(),
        "a member cannot set a credential"
    );

    // An admin with an EMPTY secret is a BadInput, not a silent empty-hash write.
    let admin = principal("user:alice", "acme", MANAGE);
    assert!(
        identity_set_credential(&store, &admin, "user:carol", "", 1)
            .await
            .is_err(),
        "an empty secret is refused"
    );
    // …and no credential was written by the failed empty-secret set.
    assert_eq!(
        credential_verify(&store, "acme", "user:carol", "")
            .await
            .unwrap(),
        CredentialCheck::Absent
    );
}
