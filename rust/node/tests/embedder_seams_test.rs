//! The **embedder seams** a downstream host binary reaches through the `lb-node` dep alone
//! (reports-server-render scope): an outbox target it registers itself, the reminder tick that makes
//! a cron schedule actually fire, and a short-lived service session it can hand to a non-interactive
//! worker. Real infra throughout — real `boot_full`, `mem://` store, real outbox enqueue, the real
//! spawned reactors. The recording target is the one sanctioned fake (a true external behind its
//! trait), exactly as `relay_boot_test`'s recording providers are.
//!
//! Each of these was previously unreachable, and each failed *silently*: a third target dead-lettered
//! with no adapter, a reminder sat at its due timestamp for ever, and a worker had no way to
//! authenticate that was not either interactive or a hand-rolled `Claims`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use lb_node::{boot_full, BootConfig, GatewayMode, OutboxEffect, Target};

/// Records the effects routed to it, so the test asserts the relay actually dispatched.
#[derive(Default)]
struct RecordingTarget {
    seen: Mutex<Vec<OutboxEffect>>,
}

impl RecordingTarget {
    fn seen(&self) -> Vec<OutboxEffect> {
        self.seen.lock().unwrap().clone()
    }
}

impl Target for RecordingTarget {
    fn deliver(
        &self,
        effect: &OutboxEffect,
    ) -> impl std::future::Future<Output = Result<(), lb_host::DeliveryError>> + Send {
        let effect = effect.clone();
        let seen = &self.seen;
        async move {
            seen.lock().unwrap().push(effect);
            Ok(())
        }
    }
}

async fn eventually(mut check: impl FnMut() -> bool) -> bool {
    for _ in 0..200 {
        if check() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    check()
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// An embedder registers a target under its OWN name; the boot-spawned relay dispatches to it.
///
/// Before this seam existed `outbox_providers` had exactly two slots (`email`, `push`), so an effect
/// addressed anywhere else hit `RouterTarget`'s "no delivery adapter" arm, retried four times and
/// dead-lettered — with the reason discarded, so the row carried no explanation either.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_embedder_registered_outbox_target_receives_its_effects() {
    let target = Arc::new(RecordingTarget::default());

    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = true;
    cfg.outbox_providers
        .targets
        .push(("report".to_string(), target.clone()));

    let running = boot_full(cfg).await.expect("boot");
    let store = running.node.store.clone();

    let p = lb_node::Principal::routed(
        "user:alice".to_string(),
        "nube".to_string(),
        vec!["mcp:outbox.enqueue:call".to_string()],
    );
    lb_node::enqueue_outbox(
        &store,
        &p,
        "nube",
        "eff-1",
        "report",
        "render",
        r#"{"reportId":"energy"}"#,
        100,
    )
    .await
    .expect("enqueue");

    assert!(
        eventually(|| !target.seen().is_empty()).await,
        "the boot-spawned relay must dispatch to the embedder's own target"
    );
    let seen = target.seen();
    assert_eq!(seen[0].target, "report");
    assert_eq!(seen[0].action, "render");
    assert_eq!(seen[0].payload, r#"{"reportId":"energy"}"#);
}

/// A due reminder fires from the boot-spawned REMINDER tick — the driver that never existed.
///
/// The reminder reactor was written, tested and exported, and then spawned by nothing: on a real node
/// an authored cron schedule displayed a "next run" it would never reach. This asserts the tick, not
/// the pass: nothing here calls `react_to_reminders`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_due_reminder_fires_from_the_boot_spawned_tick() {
    let target = Arc::new(RecordingTarget::default());

    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = true;
    cfg.outbox_providers
        .targets
        .push(("report".to_string(), target.clone()));

    let running = boot_full(cfg).await.expect("boot");
    let store = running.node.store.clone();

    // The firing runs under the AUTHOR's re-resolved caps, so the author must really hold them.
    //
    // Seeded under the BARE handle, which is how the shipped write paths store both rows:
    // `lb_host::membership_add` and the gateway's grant routes strip the `user:` prefix before they
    // touch the store, and `mint_session` strips it again to read them back. Seeding the prefixed
    // sub writes rows no production path ever writes, and none of them resolve.
    lb_authz::membership_add_raw(&store, "nube", "alice", 1)
        .await
        .unwrap();
    let p = lb_node::Principal::routed(
        "user:alice".to_string(),
        "nube".to_string(),
        vec![
            "mcp:reminder.create:call".to_string(),
            "mcp:outbox.enqueue:call".to_string(),
        ],
    );
    // Firing re-resolves the author's caps from the DURABLE grant store (not from the create-time
    // principal), so the grant has to really be there or the fire is a deny — under the bare handle,
    // per the note above. The PRINCIPAL keeps its prefixed `sub`: that is the identity, and
    // `fire_reminder` strips it for the grant lookup exactly as the session mint does.
    lb_authz::grant_assign(
        &store,
        "nube",
        &lb_authz::Subject::User("alice".to_string()),
        "mcp:outbox.enqueue:call",
    )
    .await
    .expect("grant");

    // Created against a clock two minutes in the PAST, so `next_after` lands the first slot behind
    // real `now` and the reminder is due on the very next tick. (Creating at real `now` would put the
    // first slot up to 59s out — a test that mostly waits, and flakes when it does not wait enough.)
    lb_host::reminder_create(
        &store,
        &p,
        "nube",
        "nightly-report",
        "* * * * *",
        None,
        lb_host::ReminderAction::Outbox {
            target: "report".into(),
            action: "render".into(),
            // `preset` was retired in favour of a range EXPRESSION (`range: {from: …}`), which accepts
            // the same vocabulary (`last-7-days`, `today`, `now-6h`, …) plus relative forms a fixed
            // preset list could not express. `reminder.create` rejects the old key by name, so this
            // fixture failed at creation rather than at the tick it exists to exercise.
            payload: r#"{"reportId":"energy","range":{"from":"last-7-days"}}"#.into(),
        },
        now_secs() - 120,
    )
    .await
    .expect("reminder.create");

    assert!(
        eventually(|| !target.seen().is_empty()).await,
        "the boot-spawned reminder tick must fire the schedule and the relay must deliver it"
    );
    assert_eq!(target.seen()[0].action, "render");
}

/// The service-session mint: a real, verifiable, SHORT-lived token carrying the principal's own caps.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_service_session_is_short_lived_and_carries_only_the_principals_caps() {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = false;
    // A bound gateway — the mint exists to authenticate against one.
    cfg.gateway = GatewayMode::Addr("127.0.0.1:0".parse().unwrap());

    let running = boot_full(cfg).await.expect("boot");
    let key = running.node.key();

    let minted = running
        .mint_service_session("user:alice", "nube", now_secs(), Duration::from_secs(120))
        .await
        .expect("a gateway-bearing node mints");

    // It verifies against the node's own key, and expires on the TTL we asked for, not the 12h
    // human session — that shortness is the whole reason this seam exists.
    let p = lb_auth::verify(&key, &minted.token, now_secs()).expect("token verifies now");
    assert_eq!(p.sub(), "user:alice");
    assert_eq!(p.ws(), "nube");
    assert!(
        lb_auth::verify(&key, &minted.token, now_secs() + 300).is_err(),
        "a 120s token must be expired 300s later — it must not be a 12h session"
    );

    // It grants nothing of its own: alice holds no durable grants, so the token carries only the
    // viewer floor + reach. If minting invented authority, an admin cap would be in here.
    assert!(
        !minted
            .caps
            .iter()
            .any(|c| c == "mcp:dashboard.delete_any:call"),
        "the mint must not widen beyond the principal's own caps, got {:?}",
        minted.caps
    );
}

/// A HEADLESS node (no gateway) mints nothing — there is nothing for a token to authenticate to, and
/// returning one anyway would be a credential with no door.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_headless_node_mints_no_service_session() {
    let mut cfg = BootConfig::default();
    cfg.seed_user = None;
    cfg.hello_demo = false;
    cfg.reactors = false;
    cfg.gateway = GatewayMode::Off;

    let running = boot_full(cfg).await.expect("boot headless");
    assert!(running
        .mint_service_session("user:alice", "nube", now_secs(), Duration::from_secs(120))
        .await
        .is_none());
}
