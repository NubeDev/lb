//! The **reminder reactor** — `react_to_reminders` finds enabled+due reminders, enqueues one
//! `kind="reminder-fire"` lb-jobs job per firing, dispatches the action under the stored principal
//! (re-resolved at fire time), and advances the reminder (reminders scope).
//!
//! These cover the reactor's contract through real host seams (real embedded SurrealDB + in-proc
//! Zenoh — a `Node` is booted, so multi-thread + a unique workspace per test): each action kind
//! fires against its REAL seam (channel post → a real `lb_inbox` item; MCP tool → the real tool
//! under the principal; outbox → a real `Effect` relayed via the outbox); the per-firing job id
//! makes a re-scan idempotent; `max_runs` counts down to `Done`; `enabled=false` is skipped and
//! resumes; missed firings during an outage fire exactly once on catch-up (offline/sync); the
//! mandatory capability-deny (a revoked action grant) and workspace-isolation hold across store +
//! reactor. The cron math itself is unit-tested in `lb-reminders`; here it is exercised end to end
//! on the injected logical clock.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    fire_job_id, react_to_reminders, reminder_create, reminder_update, Node, ReminderAction,
    ReminderStatus,
};
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
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// Grant `cap` to `user` in `ws` directly in the durable grant store (raw verb, no admin gate) —
/// this is how the fire-time re-resolve sees the stored principal's CURRENT caps, and how a revoke
/// (via `grant_revoke`) takes effect at the next fire.
async fn grant(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    lb_authz::grant_assign(store, ws, &lb_authz::Subject::User(user.to_string()), cap)
        .await
        .unwrap();
}

async fn revoke(store: &lb_store::Store, ws: &str, user: &str, cap: &str) {
    lb_authz::grant_revoke(store, ws, &lb_authz::Subject::User(user.to_string()), cap)
        .await
        .unwrap();
}

// Anchors: 2024-01-01 is a Monday. `* * * * *` fires every minute, so successive `now` values one
// minute apart drive clean recurring firings on the injected clock.
const MON_JAN1_0000: u64 = 1_704_067_200;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn channel_post_firing_writes_a_real_inbox_item_and_advances() {
    let ws = "react-chan";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    // Grant the action's own cap to the creator's subject — the fire-time re-resolve sees it.
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;

    // A recurring every-minute reminder; first fire is the next minute strictly after create-time.
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "standup",
        "* * * * *",
        None,
        Action::ChannelPost {
            channel: "team".into(),
            body: "standup time".into(),
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();
    let first = r.next_attempt_ts; // MON_JAN1_0000 + 60

    // At `first`, the reactor fires once: writes a real lb_inbox item + enqueues the job + advances.
    let pass = react_to_reminders(&node, ws, first).await.unwrap();
    assert_eq!(pass.fired, 1);
    assert_eq!(pass.skipped, 0);

    // The durable inbox item landed in the channel (the real seam).
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].body, "standup time");
    assert_eq!(items[0].author, "user:ada");
    assert_eq!(items[0].id, fire_job_id("standup", first));

    // The firing job is durable at the deterministic per-firing id.
    assert!(
        lb_jobs::load(&node.store, ws, &fire_job_id("standup", first))
            .await
            .unwrap()
            .is_some()
    );

    // The reminder advanced: runs=1, next_attempt_ts moved to the next future minute.
    let after = lb_reminders::load(&node.store, ws, "standup")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.runs, 1);
    assert!(after.next_attempt_ts > first);
    assert_eq!(after.status, ReminderStatus::Active); // recurring → stays active
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn mcp_tool_firing_runs_the_real_tool_under_the_stored_principal() {
    let ws = "react-tool";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    // The action calls `store.schema` — grant it to the creator so the fire-time re-check passes.
    grant(&node.store, ws, "user:ada", "mcp:store.schema:call").await;

    let action = Action::McpTool {
        tool: "store.schema".into(),
        args: serde_json::json!({}),
    };
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "schema",
        "* * * * *",
        None,
        action,
        MON_JAN1_0000,
    )
    .await
    .unwrap();

    // Firing re-enters call_tool → store.schema runs under the creator's principal (re-checked).
    let pass = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1);
    assert_eq!(pass.denied, 0, "the tool ran under the granted principal");

    let after = lb_reminders::load(&node.store, ws, "schema")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.runs, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn outbox_firing_enqueues_a_real_effect_relayed_via_the_outbox() {
    let ws = "react-outbox";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "mcp:outbox.enqueue:call").await;

    let action = Action::Outbox {
        target: "email".into(),
        action: "notify".into(),
        payload: "{\"hi\":1}".into(),
    };
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "notify",
        "* * * * *",
        None,
        action,
        MON_JAN1_0000,
    )
    .await
    .unwrap();
    let first = r.next_attempt_ts;

    let pass = react_to_reminders(&node, ws, first).await.unwrap();
    assert_eq!(pass.fired, 1);

    // A real Effect is staged in the outbox (the must-deliver seam), id derived from the firing.
    let pending = lb_outbox::pending(&node.store, ws).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].target, "email");
    assert_eq!(pending[0].action, "notify");
    assert_eq!(pending[0].id, fire_job_id("notify", first));

    // And it relays through a real target (reuse the outbox's recording Target harness shape).
    use lb_outbox::Effect;
    struct Sink(std::sync::Mutex<Vec<String>>);
    impl lb_host::Target for Sink {
        async fn deliver(&self, effect: &Effect) -> Result<(), String> {
            self.0.lock().unwrap().push(effect.id.clone());
            Ok(())
        }
    }
    let target = Sink(std::sync::Mutex::new(Vec::new()));
    let rp = lb_host::relay_outbox(&node.store, ws, &target, first + 1)
        .await
        .unwrap();
    assert_eq!(rp.delivered, 1);
    assert_eq!(target.0.lock().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_re_scan_before_advance_fires_nothing_twice() {
    // IDEMPOTENCY (mandatory): the per-firing job id is deterministic on (reminder, scheduled_ts).
    // After a firing, the job exists; a re-scan at the same/earlier time skips it — one scheduled
    // instant → one job → one effect. (The advance also moved next_attempt_ts, so a later scan
    // addresses a NEW instant.)
    let ws = "react-idem";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "r1",
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
    let first = r.next_attempt_ts;

    let p1 = react_to_reminders(&node, ws, first).await.unwrap();
    assert_eq!(p1.fired, 1);

    // Re-scan at the SAME instant (e.g. the advance raced, or a second scan): the job exists → skip.
    // (next_attempt_ts already advanced, so `due` wouldn't return it anyway; force the point by
    // re-running the pass — it must not produce a second item.)
    let p2 = react_to_reminders(&node, ws, first).await.unwrap();
    assert_eq!(p2.fired, 0, "no double-fire on re-scan");

    // Exactly one inbox item, ever.
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn max_runs_counts_down_to_done() {
    let ws = "react-maxruns";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "bounded",
        "* * * * *",
        Some(2),
        Action::ChannelPost {
            channel: "team".into(),
            body: "x".into(),
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();

    // Fire 1 → runs=1, active.
    react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    let mid = lb_reminders::load(&node.store, ws, "bounded")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(mid.runs, 1);
    assert_eq!(mid.status, ReminderStatus::Active);

    // Fire 2 → runs=2 == max_runs → Done + disabled. The next_attempt_ts no longer matters.
    react_to_reminders(&node, ws, mid.next_attempt_ts)
        .await
        .unwrap();
    let done = lb_reminders::load(&node.store, ws, "bounded")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.runs, 2);
    assert_eq!(done.status, ReminderStatus::Done);
    assert!(!done.enabled);

    // A further scan fires nothing (Done + disabled).
    let pass = react_to_reminders(&node, ws, done.next_attempt_ts + 9999)
        .await
        .unwrap();
    assert_eq!(pass.fired, 0);
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 2, "fired exactly max_runs times, then stopped");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn disabled_is_skipped_and_resumes_at_the_next_future_slot() {
    let ws = "react-enabled";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal(
        "user:ada",
        ws,
        &["mcp:reminder.create:call", "mcp:reminder.update:call"],
    );
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "paused",
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

    // Pause: the scan at the due instant fires nothing.
    reminder_update(
        &node.store,
        &creator,
        ws,
        "paused",
        lb_host::ReminderPatch {
            enabled: Some(false),
            ..Default::default()
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();
    let pass = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(pass.fired, 0, "disabled reminder is skipped");
    assert!(lb_inbox::list(&node.store, ws, "team")
        .await
        .unwrap()
        .is_empty());

    // Resume at a later `now`: re-anchors next_attempt_ts to the next future slot, then fires.
    let later = MON_JAN1_0000 + 3_600; // +1h
    reminder_update(
        &node.store,
        &creator,
        ws,
        "paused",
        lb_host::ReminderPatch {
            enabled: Some(true),
            ..Default::default()
        },
        later,
    )
    .await
    .unwrap();
    let resumed = lb_reminders::load(&node.store, ws, "paused")
        .await
        .unwrap()
        .unwrap();
    assert!(resumed.next_attempt_ts > later);
    let pass = react_to_reminders(&node, ws, resumed.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "resumed reminder fires again");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_due_during_outage_fires_exactly_once_on_catch_up() {
    // OFFLINE/SYNC (mandatory): a reminder whose `next_attempt_ts` passed during an outage (the
    // reactor did not run) fires EXACTLY ONCE on the next scan after recovery, then advances to the
    // next FUTURE slot (fire-once-then-skip — no backfill storm).
    let ws = "react-offline";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "catchup",
        "* * * * *", // every minute
        None,
        Action::ChannelPost {
            channel: "team".into(),
            body: "x".into(),
        },
        MON_JAN1_0000,
    )
    .await
    .unwrap();
    let scheduled = r.next_attempt_ts; // the instant that "passed" during the outage

    // The node was "down" for an hour; recovery happens at scheduled + 3600. Several minutes
    // elapsed, but only ONE firing should occur (no backfill of every missed minute).
    let recovery = scheduled + 3_600;
    let pass = react_to_reminders(&node, ws, recovery).await.unwrap();
    assert_eq!(pass.fired, 1, "one catch-up fire, not a backfill storm");
    let items = lb_inbox::list(&node.store, ws, "team").await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, fire_job_id("catchup", scheduled));

    // The reminder advanced to the next FUTURE slot strictly after `recovery` (not after the missed
    // instant), so the next scan won't re-fire the past.
    let after = lb_reminders::load(&node.store, ws, "catchup")
        .await
        .unwrap()
        .unwrap();
    assert!(
        after.next_attempt_ts > recovery,
        "skip-to-next-future-slot, no backfill"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_revoked_action_grant_is_a_logged_deny_with_no_effect() {
    // CAPABILITY-DENY at the firing (mandatory): the action's grant was revoked AFTER create. The
    // fire-time re-resolve sees the missing cap → the action's own gate denies. No effect produced,
    // no escalation, and the reminder is LEFT SCHEDULED (the deny is logged; the stable job id keeps
    // a re-scan from double-firing the instant).
    let ws = "react-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
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
    revoke(&node.store, ws, "user:ada", "bus:chan/team:pub").await;

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

    // The reminder is LEFT SCHEDULED (not advanced) — runs unchanged, status active, same instant.
    let after = lb_reminders::load(&node.store, ws, "revoked")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.runs, 0);
    assert_eq!(after.status, ReminderStatus::Active);
    assert_eq!(after.next_attempt_ts, r.next_attempt_ts);

    // The job for that instant exists (the attempt was recorded), so a re-scan skips it (no retry
    // storm, no re-fire of the same denied instant).
    let again = react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(again.denied, 0);
    assert_eq!(again.fired, 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_reactor_never_fires_or_advances_a_ws_a_reminder() {
    // WORKSPACE-ISOLATION across store + reactor (mandatory): ws-A has a due reminder; a reactor
    // pass over ws-B's namespace sees nothing, fires nothing, advances nothing. The hard wall holds
    // at the `due` scan.
    let node = Arc::new(Node::boot().await.unwrap());
    let a = principal("user:ada", "react-iso-a", &["mcp:reminder.create:call"]);
    grant(&node.store, "react-iso-a", "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &a,
        "react-iso-a",
        "secret",
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

    // A ws-B reactor pass fires nothing (the due scan selects ws-B's namespace — empty).
    let pass_b = react_to_reminders(&node, "react-iso-b", r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(pass_b.fired, 0, "ws-B reactor never fires a ws-A reminder");
    // ws-A's reminder is untouched (not advanced).
    let untouched = lb_reminders::load(&node.store, "react-iso-a", "secret")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(untouched.runs, 0);
    assert_eq!(untouched.next_attempt_ts, r.next_attempt_ts);
    // And no inbox item appeared in ws-B.
    assert!(lb_inbox::list(&node.store, "react-iso-b", "team")
        .await
        .unwrap()
        .is_empty());

    // ws-A's own reactor DOES fire it — proving it was genuinely due.
    let pass_a = react_to_reminders(&node, "react-iso-a", r.next_attempt_ts)
        .await
        .unwrap();
    assert_eq!(pass_a.fired, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn recurring_multi_day_schedule_on_the_injected_clock() {
    // Cron "next after T" math end to end on the injected clock: a Mon+Sun 08:00 reminder created
    // at Mon 00:00 first fires Mon 08:00, then advances to Sun 08:00 (the multi-value day field).
    let ws = "react-multiday";
    let node = Arc::new(Node::boot().await.unwrap());
    let creator = principal("user:ada", ws, &["mcp:reminder.create:call"]);
    grant(&node.store, ws, "user:ada", "bus:chan/team:pub").await;
    let r = reminder_create(
        &node.store,
        &creator,
        ws,
        "multiday",
        "0 8 * * 0,1",
        None,
        Action::ChannelPost {
            channel: "team".into(),
            body: "x".into(),
        },
        MON_JAN1_0000, // Mon 00:00
    )
    .await
    .unwrap();
    assert_eq!(r.next_attempt_ts, 1_704_096_000); // Mon 08:00

    react_to_reminders(&node, ws, r.next_attempt_ts)
        .await
        .unwrap();
    let after = lb_reminders::load(&node.store, ws, "multiday")
        .await
        .unwrap()
        .unwrap();
    // Next fire advanced to Sun 07 08:00 (2024-01-07), exercising the multi-value day field rollover.
    assert_eq!(after.next_attempt_ts, 1_704_614_400);
    assert_eq!(after.runs, 1);
    let _ = ReminderAction::ChannelPost {
        channel: "".into(),
        body: "".into(),
    }; // exercise re-export
}
