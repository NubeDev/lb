//! Invite **i18n** tests (release scope, gaps a+b): the locale rides the invite record and its
//! email effect; the email target renders subject/body through the prefs catalog engine in that
//! locale; the pre-auth `invite.verify` preview exposes it; and accept copies it into the new
//! member's `language` pref. Real store, real outbox, real relay pass, real catalog — the
//! recording email provider is the one sanctioned fake (testing §0).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    invite_accept, invite_create, invite_verify, relay_outbox, EmailTarget, InviteError,
    RecordingEmailProvider,
};
use lb_prefs::get_user_prefs;
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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const ADMIN: &[&str] = &["mcp:invite.create:call"];

/// An `es` invite's email renders the Spanish catalog subject+body (gap b) — and an `en` (default)
/// invite renders English, through the same relay pass.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn es_invite_email_renders_spanish_through_the_catalog() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        Some("es"),
        0,
        100,
    )
    .await
    .unwrap();

    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()));
    let pass = relay_outbox(&store, "acme", &target, 101).await.unwrap();
    assert_eq!(pass.delivered, 1);

    let sends = provider.sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].subject, "Te han invitado a unirte a acme");
    assert!(
        sends[0]
            .body
            .starts_with("Has sido invitado al espacio de trabajo acme"),
        "Spanish body, got: {}",
        sends[0].body
    );
    assert!(sends[0].body.contains("/accept?token=lbi_"));
}

/// No locale ⇒ the `en` fallback renders the English catalog message (never blank, never a key).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn default_invite_email_renders_english() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    invite_create(
        &store,
        &admin,
        "acme",
        "pat@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()));
    relay_outbox(&store, "acme", &target, 101).await.unwrap();

    let sends = provider.sends();
    assert_eq!(sends[0].subject, "You are invited to join acme");
    assert!(sends[0]
        .body
        .starts_with("You have been invited to workspace acme"));
}

/// An unknown locale is rejected at mint time (validated against the enabled-language axis).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_locale_is_rejected_at_create() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    let err = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        Some("xx"),
        0,
        100,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, InviteError::BadInput(_)), "got {err:?}");
}

/// The pre-auth `invite.verify` preview exposes the locale (+ email + redeemable) so the accept
/// page can render in the invite's language before any session exists (gap a).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn verify_exposes_locale_pre_auth() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        Some("es"),
        0,
        100,
    )
    .await
    .unwrap();

    let preview = invite_verify(&store, "acme", &token, 101).await.unwrap();
    assert_eq!(preview.email, "sam@example.com");
    assert_eq!(preview.locale.as_deref(), Some("es"));
    assert!(preview.redeemable);

    // A garbage token is NotFound/BadToken — the preview is token-gated like accept.
    let err = invite_verify(&store, "acme", "lbi_nope", 101)
        .await
        .unwrap_err();
    assert!(matches!(err, InviteError::BadToken | InviteError::NotFound));
}

/// Accepting an `es` invite copies the locale into the new member's `language` pref (gap a) —
/// push/UI localization reads it from first login on.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn accept_copies_locale_into_language_pref() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", ADMIN);
    let key = SigningKey::generate();

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        Some("es"),
        0,
        100,
    )
    .await
    .unwrap();

    let accepted = invite_accept(&store, &key, "acme", &token, "s3cret!", None, 101)
        .await
        .unwrap();
    assert_eq!(accepted.sub, "user:sam@example.com");

    let prefs = get_user_prefs(&store, "acme", "user:sam@example.com")
        .await
        .unwrap()
        .expect("prefs record seeded on accept");
    assert_eq!(prefs.language.as_deref(), Some("es"));
}
