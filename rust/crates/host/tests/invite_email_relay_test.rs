//! The invite email through the REAL outbox relay (invites review fix 3): `invite.create` stages
//! the must-deliver effect; `relay_outbox` — the exact loop `spawn_relay_reactors` ticks — delivers
//! it through `EmailTarget` to the `RecordingEmailProvider` (the one sanctioned fake: a true
//! external behind one trait, testing-scope §0). Everything else is real: real store, real outbox
//! enqueue, real relay pass. Wiring contract (now stated in the scope doc): delivery happens ONLY
//! if the product host registers `EmailTarget` with `spawn_relay_reactors` at boot.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invite_create, relay_outbox, EmailTarget, RecordingEmailProvider};
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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invite_create_effect_delivers_through_real_relay() {
    let store = Store::memory().await.unwrap();
    let admin = principal("user:alice", "acme", &["mcp:invite.create:call"]);

    let token = invite_create(
        &store,
        &admin,
        "acme",
        "sam@example.com",
        "member",
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();

    // The REAL relay pass (what spawn_relay_reactors ticks) delivering through the real
    // EmailTarget adapter to the recording provider.
    let provider = Arc::new(RecordingEmailProvider::default());
    let target = EmailTarget::new(Box::new(provider.clone()));
    let pass = relay_outbox(&store, "acme", &target, 101).await.unwrap();
    assert_eq!(
        pass.delivered, 1,
        "the invite email effect must be delivered"
    );

    let sends = provider.sends();
    assert_eq!(sends.len(), 1);
    assert_eq!(sends[0].to, "sam@example.com");
    assert_eq!(sends[0].workspace, "acme");
    assert!(
        sends[0].body.contains(&token),
        "the mail body must carry the raw one-time token"
    );

    // Idempotent second pass: nothing is due, nothing re-sends.
    let pass = relay_outbox(&store, "acme", &target, 102).await.unwrap();
    assert_eq!(pass.delivered, 0);
    assert_eq!(provider.sends().len(), 1);

    // Workspace wall: a ws-B relay pass never delivers ws-A's effect.
    let provider_b = Arc::new(RecordingEmailProvider::default());
    let target_b = EmailTarget::new(Box::new(provider_b.clone()));
    let pass = relay_outbox(&store, "globex", &target_b, 103)
        .await
        .unwrap();
    assert_eq!(pass.delivered, 0);
    assert!(
        provider_b.sends().is_empty(),
        "ws-B must not see ws-A's outbox"
    );
}
