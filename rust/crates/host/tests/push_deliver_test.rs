//! Deliver-path tests for the push outbox target, through the REAL outbox relay (push-target
//! scope review fixes). `notify.send` enqueues a real effect; `relay_outbox` — the deterministic
//! seam `spawn_relay_reactors` ticks in production (the product host registers `PushTarget` with
//! it at boot, the same wiring contract as `EmailTarget`) — delivers it through `PushTarget` with
//! the one sanctioned recording fake. Covers: happy-path fan-out, token-gone auto-disable,
//! quiet-hours suppression, the mandatory ws-isolation (non-member audience sub excluded), and
//! at-least-once dedup (a retry after partial failure does NOT re-send succeeded devices).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{
    device_list, device_register, notify_send, relay_outbox, PushTarget, RecordingPushProvider,
};
use lb_prefs::{set_user_prefs, Prefs};
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

/// Seed a member with a registered device; returns the device id.
async fn member_with_device(store: &Store, ws: &str, sub: &str, token: &str) -> String {
    membership_add_raw(store, ws, sub, 1).await.unwrap();
    let p = principal(sub, ws, CAPS);
    device_register(store, &p, ws, "webpush", token, None, 100)
        .await
        .unwrap();
    let devices = device_list(store, &p, ws).await.unwrap();
    devices
        .iter()
        .find(|d| d.token == token)
        .unwrap()
        .id
        .clone()
}

fn push_target(store: &Store) -> (Arc<RecordingPushProvider>, PushTarget) {
    let provider = Arc::new(RecordingPushProvider::default());
    let target = PushTarget::new(Box::new(provider.clone()), store.clone());
    (provider, target)
}

// ── (a) Happy path: fan-out records one send per live device ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn relay_fans_out_to_each_recipients_devices() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;
    member_with_device(&store, "acme", "user:bob", "bob-tablet").await;
    member_with_device(&store, "acme", "user:ana", "ana-phone").await;

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into(), "user:ana".into()],
        "Leo checked in",
        "9:00 AM",
        None,
        None,
        Some("care-feed"),
        None,
        100,
    )
    .await
    .unwrap();

    let (provider, target) = push_target(&store);
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 1, "one effect delivered");

    let sends = provider.sends();
    assert_eq!(sends.len(), 3, "one send per live device");
    assert_eq!(sends.iter().filter(|s| s.sub == "user:bob").count(), 2);
    assert_eq!(sends.iter().filter(|s| s.sub == "user:ana").count(), 1);
    // collapse_key is honored: forwarded to the provider for provider-side collapse.
    assert!(sends
        .iter()
        .all(|s| s.collapse_key.as_deref() == Some("care-feed")));
}

// ── (b) TokenGone → device auto-disabled and never retried ───────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn token_gone_auto_disables_device_and_stops_sending() {
    let store = Store::memory().await.unwrap();
    let gone_id = member_with_device(&store, "acme", "user:bob", "bob-old-tablet").await;
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;

    let sender = principal("user:staff", "acme", CAPS);
    let send = |title: &'static str, now: u64| {
        let store = store.clone();
        let sender = sender.clone();
        async move {
            notify_send(
                &store,
                &sender,
                "acme",
                &["user:bob".into()],
                title,
                "b",
                None,
                None,
                None,
                None,
                now,
            )
            .await
            .unwrap()
        }
    };
    send("first", 100).await;

    let (provider, target) = push_target(&store);
    provider.mark_token_gone(&gone_id);
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    // TokenGone is terminal for the device, not a delivery failure: the effect is delivered.
    assert_eq!(pass.delivered, 1);
    assert_eq!(provider.sends().len(), 1, "only the live device was sent");

    // The device is now disabled in the store…
    let bob = principal("user:bob", "acme", CAPS);
    let devices = device_list(&store, &bob, "acme").await.unwrap();
    assert!(devices.iter().find(|d| d.id == gone_id).unwrap().disabled);

    // …and a later notification never touches it (no retry to a gone token).
    send("second", 300).await;
    relay_outbox(&store, "acme", &target, 400).await.unwrap();
    let sends = provider.sends();
    assert_eq!(sends.len(), 2);
    assert!(sends.iter().all(|s| s.device_id != gone_id));
}

// ── (c) Quiet hours: push_muted member is suppressed ─────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn quiet_hours_suppresses_muted_member() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;
    member_with_device(&store, "acme", "user:ana", "ana-phone").await;
    set_user_prefs(
        &store,
        "acme",
        "user:ana",
        &Prefs {
            push_muted: Some(true),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into(), "user:ana".into()],
        "t",
        "b",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();

    let (provider, target) = push_target(&store);
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 1, "suppression is not a failure");
    let sends = provider.sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].sub, "user:bob");
}

// ── (d) Mandatory ws isolation: a non-member audience sub is excluded ────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_member_audience_sub_is_excluded() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;
    // Eve has a device row in acme (e.g. registered before leaving) but NO membership,
    // and a live membership + device in globex — neither may receive an acme effect.
    let eve = principal("user:eve", "acme", CAPS);
    device_register(&store, &eve, "acme", "webpush", "eve-stale", None, 100)
        .await
        .unwrap();
    member_with_device(&store, "globex", "user:eve", "eve-globex-phone").await;

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into(), "user:eve".into()],
        "t",
        "b",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();

    let (provider, target) = push_target(&store);
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 1, "exclusion is silent, not a failure");
    let sends = provider.sends();
    assert_eq!(sends.len(), 1, "only the acme member's device");
    assert_eq!(sends[0].sub, "user:bob");
}

// ── (e) At-least-once dedup: retry re-sends only the failures ────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retry_after_partial_failure_does_not_resend_succeeded_devices() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;
    let flaky_id = member_with_device(&store, "acme", "user:bob", "bob-tablet").await;

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into()],
        "t",
        "b",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();

    let (provider, target) = push_target(&store);
    provider.fail_next(&flaky_id);

    // Pass 1: one device succeeds (marked delivered), the flaky one fails → effect stays failed.
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 0);
    assert_eq!(pass.failed, 1);
    assert_eq!(provider.sends().len(), 1, "only the healthy device sent");

    // Pass 2 (past the backoff gate): ONLY the previously-failed device is re-sent.
    let pass = relay_outbox(&store, "acme", &target, 300).await.unwrap();
    assert_eq!(pass.delivered, 1);
    let sends = provider.sends();
    assert_eq!(sends.len(), 2, "no double-send to the succeeded device");
    assert_eq!(sends.iter().filter(|s| s.device_id == flaky_id).count(), 1);
}

// ── Two notify.sends in the same second are distinct effects (ULID id, no collision) ─────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn same_second_sends_do_not_collide() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;
    let sender = principal("user:staff", "acme", CAPS);

    let id1 = notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into()],
        "one",
        "b",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();
    let id2 = notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into()],
        "two",
        "b",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();
    assert_ne!(id1, id2, "same-second sends must not share an effect id");

    let (provider, target) = push_target(&store);
    relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(
        provider.sends().len(),
        2,
        "both notifications reach the device"
    );
}

// ── An effect without a workspace in its payload fails, never guesses (rule 6) ───────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn effect_missing_workspace_fails_instead_of_guessing() {
    let store = Store::memory().await.unwrap();
    member_with_device(&store, "acme", "user:bob", "bob-phone").await;

    // Hand-craft a legacy/foreign effect with no `workspace` field in the payload.
    let payload = serde_json::json!({ "to": ["user:bob"], "title": "t", "body": "b" });
    let effect = lb_outbox::Effect::new(
        "notify:bad",
        "push",
        "notify",
        &payload.to_string(),
        "notify:bad",
        100,
    );
    lb_outbox::enqueue(
        &store,
        "acme",
        "notify",
        "notify:bad",
        &serde_json::json!({}),
        &effect,
    )
    .await
    .unwrap();

    let (provider, target) = push_target(&store);
    let pass = relay_outbox(&store, "acme", &target, 200).await.unwrap();
    assert_eq!(pass.delivered, 0, "no delivery without a workspace");
    assert_eq!(pass.failed, 1);
    assert!(
        provider.sends().is_empty(),
        "nothing sent to a guessed workspace"
    );
}
