//! The **email transport** through the real relay (email-transport scope, issue #118) — the mandatory
//! test categories for the slice, plus the honest-outcome contract.
//!
//! Everything is real: a real `mem://` store, a real `invite.create` transaction, real sealed secrets,
//! and the real `relay_outbox` pass that `spawn_relay_reactors` ticks. The `RecordingEmailProvider` is
//! the one sanctioned fake (a true external behind one trait, testing §0) and it is used ONLY to observe
//! the target's behaviour; the transport itself is proven against a real SMTP server in `lb-mail`'s
//! `smtp_send_test.rs`, because asserting our own recorder says nothing about TLS/auth/MIME.
//!
//! Categories covered here:
//! - **capability deny** on the *enqueue* side — the transport exposes no verb, so the gate under test is
//!   `invite.create`. A denied call must write NO outbox row (not merely return an error).
//! - **workspace isolation** — the credential is resolved from the effect's workspace only; a secret
//!   sealed in ws B is unreachable to a ws-A effect, and an effect with no workspace fails rather than
//!   defaulting (the `push_target` hardcoded-workspace regression, cross-checked).
//! - **offline / re-drain** — a transient failure mid-relay re-drains and sends exactly once.
//! - **honest outcomes** — a permanent failure is parked on the FIRST attempt with its reason recorded;
//!   a transient one keeps its retry budget.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    invite_create, relay_outbox, DeliveryError, EmailTarget, RecordingEmailProvider,
    SmtpEmailProvider, SmtpTransportConfig,
};
use lb_outbox::{pending, EffectStatus};
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

/// Every effect row in `ws`, whatever its status (the durable ledger the assertions read).
async fn all_effects(store: &Store, ws: &str) -> Vec<lb_outbox::Effect> {
    let mut rows = pending(store, ws).await.unwrap();
    rows.extend(lb_outbox::delivered(store, ws).await.unwrap());
    rows.extend(lb_outbox::dead_lettered(store, ws).await.unwrap());
    rows
}

async fn stage_invite(store: &Store, ws: &str, email: &str, locale: Option<&str>) -> String {
    let admin = principal("user:alice", ws, &["mcp:invite.create:call"]);
    invite_create(store, &admin, ws, email, "member", "", None, locale, 0, 100)
        .await
        .expect("invite.create")
}

// ── Capability deny (the enqueue side) ────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unauthorized_invite_writes_no_outbox_row_at_all() {
    let store = Store::memory().await.unwrap();
    // Deliberately holds a DIFFERENT, real capability — so this proves the `invite.create` gate, not
    // merely "a principal with no caps is denied everything".
    let nobody = principal("user:mallory", "acme", &["mcp:outbox.status:call"]);

    let denied = invite_create(
        &store,
        &nobody,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await;
    assert!(denied.is_err(), "invite.create must be gated");

    // The load-bearing half: nothing was staged. A denied call that still wrote an effect would mail
    // the invite anyway on the next relay tick — the gate would be decorative.
    assert!(
        all_effects(&store, "acme").await.is_empty(),
        "a denied invite.create must write NO outbox row"
    );

    // And the granted caller DOES stage one — so the assertion above is about the gate, not about a
    // staging path that never works.
    stage_invite(&store, "acme", "sam@example.com", None).await;
    assert_eq!(all_effects(&store, "acme").await.len(), 1);
}

// ── Workspace isolation ───────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_a_effect_can_never_resolve_a_ws_b_sealed_credential() {
    let store = Store::memory().await.unwrap();
    let owner = principal(
        "user:ops",
        "globex",
        &[
            "secret:mail/smtp-password:write",
            "secret:mail/smtp-password:get",
        ],
    );
    // The password is sealed in GLOBEX only, at workspace visibility (the host-mediated read the relay
    // reactor uses — it carries no user principal).
    lb_secrets::set_with(
        &store,
        &owner,
        "globex",
        "mail/smtp-password",
        "hunter2hunter2",
        lb_secrets::Visibility::Workspace,
    )
    .await
    .unwrap();

    let provider = SmtpEmailProvider::new(
        SmtpTransportConfig {
            // Port 1 on localhost: nothing listens, so a credential that DOES resolve fails with a
            // connection error. That difference — "no credential" vs "connection refused" — is exactly
            // how the wall is observable without a relay to talk to.
            host: "127.0.0.1".into(),
            port: 1,
            tls: lb_host::TlsMode::None,
            username: "reports@acme.com".into(),
            secret_path: "mail/smtp-password".into(),
            from_addr: "reports@acme.com".into(),
            timeout: std::time::Duration::from_millis(500),
            ..Default::default()
        },
        store.clone(),
    );
    let message = lb_host::EmailMessage {
        to: "sam@example.com".into(),
        subject: "s".into(),
        text: "t".into(),
        ..Default::default()
    };

    // ws A (acme): the seal lives in globex, so acme has NO credential — permanent, naming the path.
    let err = <SmtpEmailProvider as lb_host::EmailProvider>::send(
        &provider,
        &message,
        &lb_host::EmailMeta {
            workspace: "acme".into(),
            action: "send_invite".into(),
        },
    )
    .await
    .expect_err("acme must not see globex's secret");
    assert!(err.permanent, "{err}");
    assert!(err.reason.contains("mail/smtp-password"), "{err}");
    assert!(
        !err.reason.contains("hunter2hunter2"),
        "the error must never carry the value: {err}"
    );

    // ws B (globex): the same path DOES resolve, so the send gets as far as the socket and fails there.
    let err = <SmtpEmailProvider as lb_host::EmailProvider>::send(
        &provider,
        &message,
        &lb_host::EmailMeta {
            workspace: "globex".into(),
            action: "send_invite".into(),
        },
    )
    .await
    .expect_err("nothing is listening on port 1");
    assert!(
        !err.permanent,
        "a connection failure is retryable, not terminal: {err}"
    );
    assert!(
        !err.reason.contains("mail/smtp-password"),
        "globex resolved the credential, so this is a transport error: {err}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_effect_without_a_workspace_is_parked_not_defaulted() {
    // The `push_target` hardcoded-workspace regression, cross-checked on the email side: a target that
    // guesses a workspace resolves another tenant's config and secrets.
    let store = Store::memory().await.unwrap();
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), store.clone());

    let effect = lb_outbox::Effect::new(
        "invite:noworkspace",
        lb_host::EMAIL_TARGET,
        "send_invite",
        serde_json::json!({ "email": "sam@example.com", "token": "lbi_x" }).to_string(),
        "invite:noworkspace",
        0,
    );
    lb_outbox::enqueue(
        &store,
        "acme",
        "invite",
        "invite:noworkspace",
        &serde_json::json!({ "email": "sam@example.com" }),
        &effect,
    )
    .await
    .unwrap();

    let pass = relay_outbox(&store, "acme", &target, 1).await.unwrap();
    assert_eq!(
        pass.dead_lettered, 1,
        "a workspace-less effect must be parked at once"
    );
    assert!(provider.sends().is_empty(), "and nothing may be sent");

    let row = &all_effects(&store, "acme").await[0];
    assert_eq!(row.status, EffectStatus::DeadLettered);
    assert_eq!(
        row.attempts, 1,
        "parked on the FIRST attempt, no retry ladder"
    );
    let reason = row.last_error.as_deref().unwrap_or_default();
    assert!(
        reason.contains("workspace"),
        "the operator must be able to read why: {reason}"
    );
}

// ── Offline / re-drain ────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_transient_failure_re_drains_and_sends_exactly_once() {
    let store = Store::memory().await.unwrap();
    let token = stage_invite(&store, "acme", "sam@example.com", None).await;
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), store.clone());

    // The relay is interrupted the way a real one is: the relay was down / the mail server refused.
    provider.fail_next(DeliveryError::transient("smtp io: connection reset"));
    let pass = relay_outbox(&store, "acme", &target, 1).await.unwrap();
    assert_eq!(pass.failed, 1);
    assert_eq!(
        pass.dead_lettered, 0,
        "a transient failure keeps its retry budget"
    );
    assert!(provider.sends().is_empty());

    let row = &all_effects(&store, "acme").await[0];
    assert_eq!(row.status, EffectStatus::Failed);
    assert_eq!(
        row.last_error.as_deref(),
        Some("smtp io: connection reset"),
        "the reason must be recorded on the row, not dropped"
    );

    // The next pass (past the backoff gate) delivers it — once.
    let pass = relay_outbox(&store, "acme", &target, 100).await.unwrap();
    assert_eq!(pass.delivered, 1);
    assert_eq!(provider.sends().len(), 1);
    assert!(provider.sends()[0].body.contains(&token));

    // And a third pass re-sends nothing (delivered is terminal; the marker backs it up).
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 0);
    assert_eq!(
        provider.sends().len(),
        1,
        "exactly once across the re-drain"
    );

    let row = &all_effects(&store, "acme").await[0];
    assert_eq!(row.status, EffectStatus::Delivered);
    assert!(
        row.last_error.is_none(),
        "a delivered row must not still carry the earlier failure"
    );
}

// ── Honest outcomes ───────────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_permanent_failure_is_parked_on_the_first_attempt_with_its_reason() {
    // Before this slice every failure was identical: retry five times with backoff, then park with no
    // recorded reason. `550 5.1.2 Host unknown` cannot be fixed by waiting, and the retries only delay
    // the row that tells an operator the address is wrong.
    let store = Store::memory().await.unwrap();
    stage_invite(&store, "acme", "sam@exmaple.com", None).await;
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), store.clone());

    provider.fail_next(DeliveryError::permanent("smtp 550: 5.1.2 Host unknown"));
    let pass = relay_outbox(&store, "acme", &target, 1).await.unwrap();
    assert_eq!(pass.dead_lettered, 1);
    assert_eq!(pass.failed, 0);

    let row = &all_effects(&store, "acme").await[0];
    assert_eq!(row.status, EffectStatus::DeadLettered);
    assert_eq!(row.attempts, 1, "no retry ladder for a permanent failure");
    assert_eq!(
        row.last_error.as_deref(),
        Some("smtp 550: 5.1.2 Host unknown")
    );

    // Terminal: a later pass does not resurrect it.
    let pass = relay_outbox(&store, "acme", &target, 1000).await.unwrap();
    assert_eq!(pass.delivered, 0);
    assert_eq!(pass.dead_lettered, 0);
    assert!(provider.sends().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_invite_mail_carries_both_body_halves_in_the_effects_locale() {
    // The HTML half is new (two catalog keys, so a translator sees both) and must follow the effect's
    // locale exactly like the text half does.
    let store = Store::memory().await.unwrap();
    stage_invite(&store, "acme", "sam@example.com", Some("es")).await;
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()), store.clone());

    relay_outbox(&store, "acme", &target, 1).await.unwrap();
    let send = provider.sends().remove(0);
    assert!(
        send.subject.starts_with("Te han invitado"),
        "{}",
        send.subject
    );
    let html = send.html.expect("an HTML alternative");
    assert!(html.contains("Aceptar la invitación"), "{html}");
    assert!(html.contains("<a href="), "{html}");
    // The Message-ID is stable and header-legal, so a receiving MTA can collapse a duplicate.
    let id = send.message_id.expect("a Message-ID");
    assert!(
        !id.contains(':'),
        "a raw colon makes the header unparseable: {id}"
    );
}
