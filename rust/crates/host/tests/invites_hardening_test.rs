//! Invites hardening (invites review fixes): the accept-race/credential-ordering regression
//! (double-redeem loses BEFORE any credential mutation), email-match takeover prevention
//! (existing identity must present `current_secret`, and a correct one binds WITHOUT a duplicate
//! identity), accept-then-first-call proves live caps with a REAL cap-gated verb (not just a
//! non-empty caps list), and resend refreshes the expiry + rotates the token new-before-old.
//! Real store, real resolver, real capability gate — no mocks (rule 9).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    credential_verify, identity_set_credential, invite_accept, invite_create, invite_list,
    invite_resend, list_members, CredentialCheck, InviteError,
};
use lb_store::Store;

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

const ADMIN: &[&str] = &[
    "mcp:invite.create:call",
    "mcp:invite.list:call",
    "mcp:identity.manage:call",
];

// ── Review fix 1: the claim is taken BEFORE any credential mutation ─────────────────────────

/// Two accepts of the same token: the second must lose at the redemption claim, BEFORE its
/// credential write — asserted by construction: the winner's password still verifies and the
/// loser's password never took. (Sequential here; the claim itself is a store-level conditional
/// CREATE, so a truly concurrent loser takes the same reject path.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn double_redeem_loses_before_credential_mutation() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        0,
        100,
    )
    .await
    .unwrap();

    invite_accept(&store, &key, "acme", &token, "winner-pass", None, 200)
        .await
        .unwrap();

    let err = invite_accept(&store, &key, "acme", &token, "loser-pass", None, 201)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::AlreadyAccepted));

    // The loser never mutated the credential: the winner's password still verifies…
    let check = credential_verify(&store, "acme", "user:sam@example.com", "winner-pass")
        .await
        .unwrap();
    assert!(
        matches!(check, CredentialCheck::Ok),
        "winner's credential must be intact"
    );
    // …and the loser's never took.
    let check = credential_verify(&store, "acme", "user:sam@example.com", "loser-pass")
        .await
        .unwrap();
    assert!(matches!(check, CredentialCheck::BadSecret));
}

// ── Review fix 7a: takeover prevention (existing identity + credential) ─────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn existing_identity_requires_current_secret() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    // Sam already exists WITH a credential in this workspace.
    lb_authz::identity_create(&store, "user:sam@example.com", Some("sam@example.com"), 10)
        .await
        .unwrap();
    identity_set_credential(&store, &admin, "user:sam@example.com", "sams-real-pass", 11)
        .await
        .unwrap();

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        0,
        100,
    )
    .await
    .unwrap();

    // No current_secret → rejected (409 at the route), and the credential is untouched.
    let err = invite_accept(&store, &key, "acme", &token, "attacker-pass", None, 200)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::IdentityExists(_)));

    // Wrong current_secret → rejected too.
    let err = invite_accept(
        &store,
        &key,
        "acme",
        &token,
        "attacker-pass",
        Some("wrong-guess"),
        201,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, InviteError::IdentityExists(_)));

    // Both rejects happened pre-claim AND pre-mutation: the invite is still pending and the
    // original credential still verifies (the attacker's never took).
    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    assert_eq!(invites[0].status, lb_authz::InviteStatus::Pending);
    let check = credential_verify(&store, "acme", "user:sam@example.com", "sams-real-pass")
        .await
        .unwrap();
    assert!(matches!(check, CredentialCheck::Ok));

    // Correct current_secret → binds, and WITHOUT a duplicate identity.
    let accepted = invite_accept(
        &store,
        &key,
        "acme",
        &token,
        "sams-new-pass",
        Some("sams-real-pass"),
        202,
    )
    .await
    .unwrap();
    assert_eq!(accepted.sub, "user:sam@example.com");
    let identities = lb_authz::identity_list(&store).await.unwrap();
    let sams = identities
        .iter()
        .filter(|i| i.sub == "user:sam@example.com")
        .count();
    assert_eq!(
        sams, 1,
        "accept must match the existing identity, not duplicate it"
    );
}

// ── Review fix 7b: accept-then-first-call is a REAL cap-gated call ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accept_then_first_call_passes_a_real_cap_gate() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        0,
        100,
    )
    .await
    .unwrap();

    let accepted = invite_accept(&store, &key, "acme", &token, "password123", None, 200)
        .await
        .unwrap();

    // The minted token IS the session: verify it with the same key and make a real cap-gated
    // call through the normal authorize_tool path (`mcp:members.list:call` is a member cap).
    let sam = verify(&key, &accepted.token, 201).expect("minted session token verifies");
    let members = list_members(&store, &sam, "acme", "some-team")
        .await
        .expect("first call after accept must pass the cap gate without re-login");
    assert!(members.is_empty());
}

// ── Review fix 5: resend refreshes expiry, rotates new-before-old ───────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resend_refreshes_expiry_old_token_dead_new_token_works() {
    let store = Store::memory().await.unwrap();
    let key = SigningKey::generate();
    let admin = principal("user:alice", "acme", ADMIN);

    // Created at 100, expires at 200 (TTL 100).
    let old_token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        200,
        100,
    )
    .await
    .unwrap();

    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    let old_hash = invites[0].token_hash.clone();

    // Resend at 180 (invite nearly expired): the new invite gets the SAME TTL from now.
    let new_token = invite_resend(&store, &admin, "acme", &old_hash, 180)
        .await
        .unwrap();
    assert_ne!(new_token, old_token, "resend must rotate the token");

    let invites = invite_list(&store, &admin, "acme").await.unwrap();
    let pending: Vec<_> = invites
        .iter()
        .filter(|i| i.status == lb_authz::InviteStatus::Pending)
        .collect();
    assert_eq!(pending.len(), 1, "exactly one pending invite after resend");
    assert_eq!(pending[0].created_ts, 180);
    assert_eq!(
        pending[0].expires_ts, 280,
        "expiry = original TTL measured from resend time"
    );

    // The old token is dead.
    let err = invite_accept(&store, &key, "acme", &old_token, "password123", None, 190)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::Revoked));

    // The new token works — even past the ORIGINAL expiry (the refresh is the point).
    let accepted = invite_accept(&store, &key, "acme", &new_token, "password123", None, 250)
        .await
        .unwrap();
    assert_eq!(accepted.sub, "user:sam@example.com");
}
