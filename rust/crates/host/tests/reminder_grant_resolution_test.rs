//! **What the author's grants do at a reminder firing** — the fire-time re-resolve, and what a
//! denial does to the schedule afterwards. Split out of `reminders_reactor_test.rs` (FILE-LAYOUT
//! §3): that file covers the reactor's SCHEDULING behaviour (advance, skip, catch-up, max-runs,
//! workspace isolation); this one covers AUTHORIZATION at the moment of firing.
//!
//! Both tests here guard bugs that made every user-authored reminder unfireable, and both were
//! previously masked by fixtures that seeded grants under a key production never writes. Note the
//! shared `subject()` helper: it PARSES the principal sub exactly as the `grants.assign` verb does,
//! so a grant here lands where a real grant lands. That detail is the whole point.
//!
//! All real: an embedded `Node` (real SurrealDB `mem://` + Zenoh), real grants, the real shipped
//! reactor and fire path — no mocks (testing §0).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{react_to_reminders, reminder_create, Node, ReminderStatus};
use lb_reminders::Action;

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

/// Grant `cap` to `user` in `ws` directly in the durable grant store (raw verb, no admin gate) —
/// this is how the fire-time re-resolve sees the stored principal's CURRENT caps, and how a revoke
/// (via `grant_revoke`) takes effect at the next fire.
///
/// **`user` is a principal `sub` and is PARSED**, exactly as the `grants.assign` verb parses it, so
/// the row lands under the bare handle the store keys on. Wrapping the sub verbatim wrote a key
/// nothing in production writes, which let these tests pass against a fire path that resolved zero
/// caps for every real user.
fn subject(user: &str) -> lb_authz::Subject {
    lb_authz::Subject::parse(user).expect("a well-formed subject like `user:test`")
}

async fn grant(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    lb_authz::grant_assign(store, ws, &subject(user), cap)
        .await
        .unwrap();
}

async fn revoke(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    lb_authz::grant_revoke(store, ws, &subject(user), cap)
        .await
        .unwrap();
}

// Anchors: 2024-01-01 is a Monday. `* * * * *` fires every minute, so successive `now` values one
// minute apart drive clean recurring firings on the injected clock.
const MON_JAN1_0000: u64 = 1_704_067_200;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_revoked_action_grant_is_a_logged_deny_with_no_effect() {
    // CAPABILITY-DENY at the firing (mandatory): the action's grant was revoked AFTER create. The
    // fire-time re-resolve sees the missing cap → the action's own gate denies. No effect produced,
    // no escalation, and the reminder is LEFT SCHEDULED (the deny is logged; the stable job id keeps
    // a re-scan from double-firing the instant).
    let ws = "react-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:test", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:test", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "revoked",
        "* * * * *",
        None,
        Action::ChannelPost {
            channel: "team".into(),
            body: "x".into(),
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();

    // Revoke the action grant AFTER create — the principal no longer holds it.
    revoke(&node.store, ws, "user:test", "bus:chan/team:pub").await;

    let pass = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(
        pass.denied, 1,
        "the firing was denied at the action's own gate"
    );
    assert_eq!(pass.fired, 0, "no effect produced");

    // NO inbox item landed (the action never ran — no escalation).
    assert!(lb_inbox::list(&node.store, ws, "team")
        .await
        .unwrap()
        .is_empty());

    // `runs` is NOT bumped (nothing ran) and the reminder stays active — but it DOES move on to its
    // next slot. That last part is the fix: leaving `next_attempt_ts` frozen looked like "the
    // reminder waits for the grant to come back", and was in fact permanent death — the idempotency
    // job for that instant already exists, so every later scan re-selected the same instant, skipped
    // it, and the schedule never fired again. Silently, for ever.
    let after = lb_reminders::load(&node.store, ws, "revoked")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.runs, 0, "a denied firing ran nothing");
    assert_eq!(after.status, ReminderStatus::Active);
    assert!(
        after.next_attempt_ts > r.next_attempt_ts,
        "a denied firing must advance to the next slot, not freeze on the one it was refused"
    );

    // The denied instant is not re-fired (the job records the attempt) — no retry storm.
    let again = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(again.denied, 0);
    assert_eq!(again.fired, 0);

    // THE REGRESSION: restore the grant, and the very next slot fires for real. Before the fix this
    // reminder was unrecoverable — no grant, no operator action and no restart could revive it.
    grant(&node.store, ws, "user:test", "bus:chan/team:pub").await;
    let recovered = react_to_reminders(&node, ws, after.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(
        recovered.fired, 1,
        "a schedule must survive a denial it has since had fixed"
    );
    assert_eq!(
        lb_inbox::list(&node.store, ws, "team").await.unwrap().len(),
        1
    );
}
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_firing_resolves_the_authors_grants_under_the_bare_handle_the_store_keys_on() {
    // THE bug this guards: `Principal::sub()` is `"user:test"`, the durable grant store keys on the
    // BARE `"test"` (`Subject::User`), and the fire path used to hand the prefixed sub straight to
    // the resolver. It found nothing, every action was denied, and the effect was total — no
    // reminder authored by any real user had ever fired, for any action kind. It went unnoticed
    // because the tests here seeded their grants under the prefixed key too, so the fixture agreed
    // with the bug.
    //
    // This test writes the grant EXACTLY as the `grants.assign` verb does (parse the subject) and
    // asserts a real firing lands its real effect. It fails against the unfixed resolver.
    let node = Arc::new(Node::boot().await.unwrap());
    let ws = "react-bare-handle";
    let creator = principal("user:test", ws, &["mcp:reminder.create:call"]);

    lb_authz::grant_assign(
        &node.store,
        ws,
        &lb_authz::Subject::User("test".into()), // the bare handle — what production stores
        "bus:chan/team:pub",
    )
    .await
    .unwrap();

    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "bare",
        "* * * * *",
        None,
        Action::ChannelPost {
            channel: "team".into(),
            body: "hello".into(),
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();

    let pass = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(
        pass.fired, 1,
        "the author's stored grant must resolve at fire time"
    );
    assert_eq!(pass.denied, 0);
    assert_eq!(
        lb_inbox::list(&node.store, ws, "team").await.unwrap().len(),
        1,
        "the action actually ran"
    );
}
