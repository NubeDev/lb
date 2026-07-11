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
};
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
    let admin = principal("user:alice", "acme", &["mcp:invite.create:call"]);
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
    .expect("invite.create");

    assert!(
        eventually(|| !email.sends().is_empty()).await,
        "the boot-spawned relay reactor must deliver the invite email"
    );
    let sends = email.sends();
    assert_eq!(sends[0].to, "sam@example.com");
    assert_eq!(sends[0].workspace, "acme");

    // PUSH: a member with a live device; notify.send stages the effect; the same spawned relay
    // (RouterTarget route "push") delivers it.
    membership_add_raw(&store, "acme", "user:bob", 1)
        .await
        .unwrap();
    let bob = principal(
        "user:bob",
        "acme",
        &["mcp:device.register:call", "mcp:notify.send:call"],
    );
    device_register(&store, &bob, "acme", "webpush", "bob-endpoint", None, 100)
        .await
        .unwrap();
    notify_send(
        &store,
        &bob,
        "acme",
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

    let admin = principal("user:alice", "acme", &["mcp:invite.create:call"]);
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
    .expect("invite.create");

    // The logging provider acks, so the effect leaves the pending set (drained, not stranded).
    // Probe the durable due set directly on a cadence (an async probe, so no `eventually` here).
    let reader = principal("user:alice", "acme", &["mcp:outbox.due:call"]);
    let mut ok = false;
    for _ in 0..150 {
        let due = lb_host::outbox_due(&store, &reader, "acme", None, u64::MAX)
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
