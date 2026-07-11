//! Push **i18n** test (release scope, gap c): `notify.send` carries a catalog key + args; the
//! `PushTarget` renders per-recipient in each recipient's `language` pref at deliver time — ONE
//! notify to an `en` member and an `es` member yields two differently-localized payloads through
//! the real relay. Also: the literal compat path still delivers untranslated.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::membership_add_raw;
use lb_host::{
    device_register, notify_send, relay_outbox, NotifyCatalogRef, PushTarget, RecordingPushProvider,
};
use lb_prefs::{set_user_prefs, Prefs};
use lb_store::Store;
use serde_json::json;

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

const CAPS: &[&str] = &["mcp:device.register:call", "mcp:notify.send:call"];

async fn member_with_device_and_lang(store: &Store, ws: &str, sub: &str, lang: Option<&str>) {
    membership_add_raw(store, ws, sub, 1).await.unwrap();
    let p = principal(sub, ws, CAPS);
    device_register(store, &p, ws, "webpush", &format!("{sub}-dev"), None, 100)
        .await
        .unwrap();
    if let Some(l) = lang {
        let patch = Prefs {
            language: Some(l.to_string()),
            ..Default::default()
        };
        set_user_prefs(store, ws, sub, &patch).await.unwrap();
    }
}

/// ONE notify with a catalog key, two members with `en`/`es` prefs → two localized payloads on
/// the recording provider (the built-in `notify.welcome` key: "Welcome, {name}!" / "¡Bienvenido,
/// {name}!").
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_notify_renders_per_recipient_language() {
    let store = Store::memory().await.unwrap();
    member_with_device_and_lang(&store, "acme", "user:bob", None).await; // en (fallback)
    member_with_device_and_lang(&store, "acme", "user:ana", Some("es")).await;

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:bob".into(), "user:ana".into()],
        "", // no literal — the key is the message
        "",
        Some(NotifyCatalogRef {
            title_key: "notify.welcome",
            body_key: "",
            args: json!({ "name": "Sam" }),
        }),
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();

    let provider = Arc::new(RecordingPushProvider::default());
    let target = PushTarget::new(Box::new(provider.clone()), store.clone());
    let pass = relay_outbox(&store, "acme", &target, 101).await.unwrap();
    assert_eq!(pass.delivered, 1);

    let sends = provider.sends();
    assert_eq!(sends.len(), 2, "one device per member");
    let bob = sends.iter().find(|s| s.sub == "user:bob").unwrap();
    let ana = sends.iter().find(|s| s.sub == "user:ana").unwrap();
    assert_eq!(bob.title, "Welcome, Sam!");
    assert_eq!(ana.title, "¡Bienvenido, Sam!");
}

/// The literal compat path: no catalog key ⇒ every recipient gets the identical untranslated
/// strings (literals are never translated).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn literal_notify_stays_untranslated() {
    let store = Store::memory().await.unwrap();
    member_with_device_and_lang(&store, "acme", "user:ana", Some("es")).await;

    let sender = principal("user:staff", "acme", CAPS);
    notify_send(
        &store,
        &sender,
        "acme",
        &["user:ana".into()],
        "Literal title",
        "Literal body",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap();

    let provider = Arc::new(RecordingPushProvider::default());
    let target = PushTarget::new(Box::new(provider.clone()), store.clone());
    relay_outbox(&store, "acme", &target, 101).await.unwrap();

    let sends = provider.sends();
    assert_eq!(sends[0].title, "Literal title");
    assert_eq!(sends[0].body, "Literal body");
}

/// Deny-shape guard: a notify with NEITHER a literal title nor a title key is BadInput.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn notify_requires_title_or_key() {
    let store = Store::memory().await.unwrap();
    member_with_device_and_lang(&store, "acme", "user:ana", None).await;
    let sender = principal("user:staff", "acme", CAPS);
    let err = notify_send(
        &store,
        &sender,
        "acme",
        &["user:ana".into()],
        "",
        "",
        None,
        None,
        None,
        None,
        100,
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lb_host::NotifyError::BadInput(_)));
}
