//! Push-target at the host layer: device registration, notify.send, and the push Target adapter
//! (push-target scope). Tests the mandatory capability-deny, workspace isolation, self-only
//! device management, and upsert idempotency. Real store, real outbox — the provider is the one
//! sanctioned fake (RecordingPushProvider).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    device_list, device_register, device_remove, notify_send, NotifyError, PushPayload,
    PushProvider, RecordingPushProvider,
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

const CAPS: &[&str] = &["mcp:device.register:call", "mcp:notify.send:call"];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn register_and_list_device() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);

    device_register(&store, &p, "acme", "webpush", "sub-endpoint-1", None, 100)
        .await
        .unwrap();

    let devices = device_list(&store, &p, "acme").await.unwrap();
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].sub, "user:alice");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn register_is_idempotent_upsert() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);

    device_register(&store, &p, "acme", "webpush", "sub-1", None, 100)
        .await
        .unwrap();
    device_register(&store, &p, "acme", "webpush", "sub-1", None, 200)
        .await
        .unwrap();

    let devices = device_list(&store, &p, "acme").await.unwrap();
    assert_eq!(devices.len(), 1, "re-register upserts, not duplicates");
    assert_eq!(devices[0].last_seen, 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remove_own_device() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);

    device_register(&store, &p, "acme", "webpush", "sub-1", None, 100)
        .await
        .unwrap();
    let devices = device_list(&store, &p, "acme").await.unwrap();
    let id = &devices[0].id;

    let removed = device_remove(&store, &p, "acme", id).await.unwrap();
    assert!(removed);
}

// ── Mandatory: capability-deny ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_register_without_cap() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:mallory", "acme", &[]);

    let err = device_register(&store, &p, "acme", "webpush", "sub-1", None, 100)
        .await
        .unwrap_err();
    assert!(matches!(err, NotifyError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_notify_send_without_cap() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:mallory", "acme", &["mcp:device.register:call"]);

    let err = notify_send(
        &store,
        &p,
        "acme",
        &["user:bob".into()],
        "hi",
        "body",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, NotifyError::Denied));
}

// ── Mandatory: workspace isolation ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn device_not_visible_from_other_workspace() {
    let store = Store::memory().await.unwrap();
    let p_a = principal("user:alice", "acme", CAPS);

    device_register(&store, &p_a, "acme", "webpush", "sub-1", None, 100)
        .await
        .unwrap();

    let p_b = principal("user:alice", "globex", CAPS);
    let devices = device_list(&store, &p_b, "globex").await.unwrap();
    assert!(devices.is_empty());
}

// ── Self-only: cannot remove another member's device ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_removing_another_members_device() {
    let store = Store::memory().await.unwrap();
    let p_a = principal("user:alice", "acme", CAPS);
    let p_b = principal("user:bob", "acme", CAPS);

    device_register(&store, &p_a, "acme", "webpush", "alice-sub", None, 100)
        .await
        .unwrap();
    let devices = device_list(&store, &p_a, "acme").await.unwrap();
    let alice_device_id = devices[0].id.clone();

    let err = device_remove(&store, &p_b, "acme", &alice_device_id)
        .await
        .unwrap_err();
    assert!(matches!(err, NotifyError::Denied));
}

// ── notify.send enqueues an outbox effect ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn notify_send_enqueues_effect() {
    let store = Store::memory().await.unwrap();
    let p = principal("user:alice", "acme", CAPS);

    let effect_id = notify_send(
        &store,
        &p,
        "acme",
        &["user:bob".into()],
        "Leo checked in",
        "9:00 AM",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();
    assert!(!effect_id.is_empty());
}

// ── Push provider records sends ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn recording_provider_records_sends() {
    let provider = RecordingPushProvider::default();
    let device = lb_host::NotifyDevice::new(
        "user:bob",
        lb_host::DevicePlatform::Webpush,
        "bob-endpoint",
        100,
    );
    let payload = PushPayload {
        to: vec!["user:bob".into()],
        title: "Hello".into(),
        body: "World".into(),
        title_key: None,
        body_key: None,
        args: serde_json::Value::Null,
        deep_link: None,
        collapse_key: None,
        priority: None,
        workspace: Some("acme".into()),
    };
    provider.send(&device, &payload).await.unwrap();
    let sends = provider.sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].sub, "user:bob");
    assert_eq!(sends[0].title, "Hello");
}
