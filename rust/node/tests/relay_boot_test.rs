//! The **relay boot wiring** test (release scope, gap 1 — the blocker): boot a real node through
//! `boot_full` with reactors ON and recording providers injected through the `OutboxProviders`
//! seam, then prove the staged outbox effects are drained **by the spawned relay reactor** — not
//! by calling `relay_outbox` directly. `invite.create` → the recording email provider receives
//! the send; `notify.send` → the recording push provider receives the send. Real infra: `mem://`
//! store, real outbox enqueue, the real boot ritual; the recording providers are the one
//! sanctioned fake (a true external behind its trait, testing §0).

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{
    device_register, invite_create, notify_send, RecordingEmailProvider, RecordingPushProvider,
    SmtpTransportConfig,
};
use lb_node::mail::EmailTransport;
use lb_node::{boot_full, BootConfig};

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

/// Poll `check` until it returns true or ~15s elapse (the relay ticks every 2s; the first tick
/// fires immediately, so the effect normally lands well inside one period).
async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..150 {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    check()
}

/// Boot with reactors ON and both recording providers injected; the spawned relay reactor — not a
/// direct `relay_outbox` call — must drain an invite email AND a push notification.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn booted_node_drains_email_and_push_through_the_spawned_relay() {
    let email = Arc::new(RecordingEmailProvider::default());
    let push = Arc::new(RecordingPushProvider::default());

    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = true; // the point of the test: the boot-spawned relay does the draining.
    cfg.outbox_providers.email = Some(email.clone());
    cfg.outbox_providers.push = Some(push.clone());

    let running = boot_full(cfg).await.expect("boot");
    let store = running.node.store.clone();

    // EMAIL: mint an invite — the effect is staged transactionally; ONLY the spawned reactor
    // delivers it (nothing here calls relay_outbox).
    let admin = principal("user:alice", "nube", &["mcp:invite.create:call"]);
    invite_create(
        &store,
        &admin,
        "nube",
        "sam@example.com",
        "member",
        "",
        None,
        Some("es"),
        0,
        100,
    )
    .await
    .expect("invite.create");

    assert!(
        eventually(|| !email.sends().is_empty()).await,
        "the boot-spawned relay reactor must deliver the invite email"
    );
    let sends = email.sends();
    assert_eq!(sends[0].to, "sam@example.com");
    assert_eq!(sends[0].workspace, "nube");

    // PUSH: a member with a live device; notify.send stages the effect; the same spawned relay
    // (RouterTarget route "push") delivers it.
    membership_add_raw(&store, "nube", "user:bob", 1)
        .await
        .unwrap();
    let bob = principal(
        "user:bob",
        "nube",
        &["mcp:device.register:call", "mcp:notify.send:call"],
    );
    device_register(&store, &bob, "nube", "webpush", "bob-endpoint", None, 100)
        .await
        .unwrap();
    notify_send(
        &store,
        &bob,
        "nube",
        &["user:bob".into()],
        "Hello",
        "World",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .expect("notify.send");

    assert!(
        eventually(|| !push.sends().is_empty()).await,
        "the boot-spawned relay reactor must deliver the push"
    );
    let psends = push.sends();
    assert_eq!(psends[0].sub, "user:bob");
    assert_eq!(psends[0].title, "Hello");
}

/// With NO providers configured (the default), boot must not crash and the relay must still
/// drain (logging no-op providers ack) — an unconfigured node never strands its outbox.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn booted_node_without_providers_still_boots_and_drains() {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = true;

    let running = boot_full(cfg).await.expect("boot without providers");
    let store = running.node.store.clone();

    let admin = principal("user:alice", "nube", &["mcp:invite.create:call"]);
    invite_create(
        &store,
        &admin,
        "nube",
        "pat@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .expect("invite.create");

    // The logging provider acks, so the effect leaves the pending set (drained, not stranded).
    // Probe the durable due set directly on a cadence (an async probe, so no `eventually` here).
    let reader = principal("user:alice", "nube", &["mcp:outbox.due:call"]);
    let mut ok = false;
    for _ in 0..150 {
        let due = lb_host::outbox_due(&store, &reader, "nube", None, u64::MAX)
            .await
            .unwrap();
        if due.is_empty() {
            ok = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    assert!(
        ok,
        "the logging no-op provider must ack so the outbox drains"
    );
}

/// A node configured with `kind: "smtp"` against an **unreachable** relay must still boot (never crash
/// for lack of a mail server) and its effects must stay owed — retried, not acked and dropped.
///
/// This is the other half of issue #118's lesson: the old behaviour "acked" a send that never happened,
/// so the effect left the pending set and nobody could tell. With a real transport the effect is still
/// there afterwards, which is exactly what an operator needs to see.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_node_with_an_unreachable_smtp_relay_boots_and_keeps_its_effects_owed() {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = true;
    cfg.email_transport = Some(EmailTransport::Smtp(SmtpTransportConfig {
        // Port 1 on localhost: nothing listens, so every attempt fails fast with a connection error
        // (transient) rather than hanging the relay tick.
        host: "127.0.0.1".into(),
        port: 1,
        tls: lb_host::TlsMode::None,
        auth: lb_host::MailAuthMechanism::None,
        from_addr: "reports@nube.com".into(),
        timeout: Duration::from_millis(500),
        ..Default::default()
    }));

    let running = boot_full(cfg)
        .await
        .expect("boot with an unreachable relay");
    let store = running.node.store.clone();

    let admin = principal("user:alice", "nube", &["mcp:invite.create:call"]);
    invite_create(
        &store,
        &admin,
        "nube",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .expect("invite.create");

    // Give the relay reactor several ticks, then assert the effect was NOT acked away: it is still
    // schedulable (failed-with-backoff), and its row records why.
    let reader = principal("user:alice", "nube", &["mcp:outbox.due:call"]);
    let mut attempted = None;
    for _ in 0..100 {
        let rows = lb_outbox::pending(&store, "nube").await.unwrap();
        if let Some(row) = rows.into_iter().find(|e| e.attempts > 0) {
            attempted = Some(row);
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    let row = attempted.expect("the relay must have attempted (and failed) the send");
    assert_eq!(row.status, lb_outbox::EffectStatus::Failed);
    let reason = row.last_error.as_deref().unwrap_or_default();
    assert!(
        reason.contains("smtp"),
        "the failure reason must reach the row: {reason}"
    );
    // Still owed: `due` at a far-future `now` returns it, so no mail was silently lost.
    let due = lb_host::outbox_due(&store, &reader, "nube", None, u64::MAX)
        .await
        .unwrap();
    assert!(
        due.iter().any(|e| e.id == row.id),
        "an undeliverable email must stay owed, never be acked away"
    );
}
