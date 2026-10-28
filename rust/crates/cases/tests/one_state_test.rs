//! One state at a time (Insights review 2026-10): a hold is a state like the rest, so holding work
//! nobody had started puts it in hand, and moving a held case on — a stage change, a hand-over, an
//! email — ends the hold, SLA pause and all. And `waiting_on_po` is retired: a stored record that
//! still says it reads as `actioned`. Against a real store.

use lb_cases::{
    append_event, assign, get, open, set_deadlines, set_waiting_on, settle_one_state, snooze,
    workflow, workflow_with_reason, Deadlines, EventKind, Grouping, OpenInput, Resolution,
    WaitingOn, Workflow, CASE_TABLE,
};
use lb_store::Store;

const WS: &str = "ops";
const HOUR: u64 = 3_600_000;
const OPENED: u64 = 1_000;
const DUE: u64 = OPENED + 8 * HOUR;

/// An Open case with a deadline, so a dated snooze pauses its SLA.
async fn clocked(store: &Store) -> String {
    let input = OpenInput {
        title: "a job".into(),
        grouping: Grouping::Single,
        primary_insight: "ins-1".into(),
        severity: "warning".into(),
        ..Default::default()
    };
    let id = open(store, WS, input, "user:test", OPENED)
        .await
        .unwrap()
        .id;
    let deadlines = Deadlines {
        respond_by: None,
        due_at: Some(DUE),
    };
    set_deadlines(
        store,
        WS,
        &id,
        Some("p1"),
        deadlines,
        "system:sla-clock",
        OPENED,
    )
    .await
    .unwrap();
    id
}

/// Snooze `id` at `ts` until a day on: a dated hold, so the SLA pauses.
async fn hold(store: &Store, id: &str, ts: u64) {
    snooze(
        store,
        WS,
        id,
        ts + 24 * HOUR,
        Some("Waiting on parts"),
        "user:sam",
        ts,
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn holding_open_work_puts_it_in_hand() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    hold(&store, &id, OPENED + HOUR).await;
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.workflow, Workflow::Actioned);
    assert!(case.snooze_until.is_some());
    assert!(case.sla_paused_at.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn closing_a_held_case_ends_the_hold_and_its_pause() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    hold(&store, &id, OPENED + HOUR).await;
    let closed = workflow_with_reason(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        Some("Fixed remotely"),
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    assert_eq!(closed.workflow, Workflow::Resolved);
    assert_eq!(closed.snooze_until, None);
    assert_eq!(closed.snooze_reason, None);
    assert_eq!(closed.sla_paused_at, None);
    // The hour on hold is credited, as any ended pause is.
    assert_eq!(closed.sla_paused_ms, HOUR);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn handing_a_held_case_over_ends_the_hold_and_keeps_its_stage() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    hold(&store, &id, OPENED + HOUR).await;
    assign(
        &store,
        WS,
        &id,
        Some("user:priya"),
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.assigned_to.as_deref(), Some("user:priya"));
    assert_eq!(case.snooze_until, None);
    assert_eq!(case.workflow, Workflow::Actioned);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn emailing_a_held_case_ends_the_hold() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    hold(&store, &id, OPENED + HOUR).await;
    let case = set_waiting_on(
        &store,
        WS,
        &id,
        WaitingOn::Contractor,
        Some("user:sam"),
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    assert_eq!(case.snooze_until, None);
    assert_eq!(case.waiting_on, Some(WaitingOn::Contractor));
    assert_eq!(case.emailed_by.as_deref(), Some("user:sam"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_move_to_the_same_stage_is_not_a_change_and_keeps_the_hold() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    hold(&store, &id, OPENED + HOUR).await;
    let case = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    assert!(case.snooze_until.is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn holding_an_emailed_case_ends_the_email_out() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    set_waiting_on(
        &store,
        WS,
        &id,
        WaitingOn::Contractor,
        Some("user:test"),
        OPENED + HOUR,
    )
    .await
    .unwrap();
    hold(&store, &id, OPENED + 2 * HOUR).await;
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert!(case.snooze_until.is_some());
    assert_eq!(case.waiting_on, None);
    assert_eq!(case.emailed_by, None);
}

#[test]
fn a_stored_waiting_on_po_reads_as_actioned() {
    let w: Workflow = serde_json::from_str("\"waiting_on_po\"").unwrap();
    assert_eq!(w, Workflow::Actioned);
    assert_eq!(serde_json::to_string(&w).unwrap(), "\"actioned\"");
}

/// A case stored before one-state: emailed (lb recorded `emailed_by`) AND on hold. Written straight
/// to the store, because today's verbs can no longer produce one.
async fn two_states(store: &Store, held_at: u64, emailed_at: u64, insight: &str) -> String {
    let input = OpenInput {
        title: "a job".into(),
        grouping: Grouping::Single,
        primary_insight: insight.into(),
        severity: "warning".into(),
        ..Default::default()
    };
    let id = open(store, WS, input, "user:test", OPENED)
        .await
        .unwrap()
        .id;
    // Email first or hold first, as the history says.
    hold(store, &id, held_at).await;
    // The send the host records beside the email (`case.request.send`).
    append_event(
        store,
        WS,
        &id,
        EventKind::RequestSent,
        "user:test",
        serde_json::json!({}),
        emailed_at,
    )
    .await
    .unwrap();
    let mut case = get(store, WS, &id).await.unwrap().unwrap();
    case.waiting_on = Some(WaitingOn::Contractor);
    case.emailed_by = Some("user:test".into());
    case.snooze_until = Some(held_at + 24 * HOUR);
    case.snooze_reason = Some("Waiting on parts".into());
    let value = serde_json::to_value(&case).unwrap();
    lb_store::write(store, WS, CASE_TABLE, &id, &value)
        .await
        .unwrap();
    id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn settling_keeps_the_later_of_hold_and_email() {
    let store = Store::memory().await.unwrap();
    let held_last = two_states(&store, OPENED + 2 * HOUR, OPENED + HOUR, "ins-a").await;
    let emailed_last = two_states(&store, OPENED + HOUR, OPENED + 2 * HOUR, "ins-b").await;
    let now = OPENED + 3 * HOUR;
    assert_eq!(settle_one_state(&store, WS, now).await.unwrap(), 2);

    let held = get(&store, WS, &held_last).await.unwrap().unwrap();
    assert!(held.snooze_until.is_some(), "the later hold stands");
    assert_eq!(held.waiting_on, None);
    assert_eq!(held.emailed_by, None);

    let out = get(&store, WS, &emailed_last).await.unwrap().unwrap();
    assert_eq!(out.snooze_until, None, "the later email ends the hold");
    assert_eq!(out.waiting_on, Some(WaitingOn::Contractor));

    assert_eq!(
        settle_one_state(&store, WS, now).await.unwrap(),
        0,
        "idempotent"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unassigning_work_nothing_else_holds_returns_it_to_the_queue() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    assign(
        &store,
        WS,
        &id,
        Some("user:priya"),
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    assign(&store, WS, &id, None, "user:sam", OPENED + 2 * HOUR)
        .await
        .unwrap();
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.assigned_to, None);
    assert_eq!(case.workflow, Workflow::ToAction);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unassigning_work_out_by_email_keeps_it_actioned() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    assign(
        &store,
        WS,
        &id,
        Some("user:priya"),
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    set_waiting_on(
        &store,
        WS,
        &id,
        WaitingOn::Contractor,
        Some("user:sam"),
        OPENED + HOUR,
    )
    .await
    .unwrap();
    assign(&store, WS, &id, None, "user:sam", OPENED + 2 * HOUR)
        .await
        .unwrap();
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.workflow, Workflow::Actioned);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn closing_ends_the_email_out_so_a_reopen_starts_clean() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    set_waiting_on(
        &store,
        WS,
        &id,
        WaitingOn::Contractor,
        Some("user:sam"),
        OPENED + HOUR,
    )
    .await
    .unwrap();
    let closed = workflow_with_reason(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        Some("Fixed on site"),
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    assert_eq!(closed.waiting_on, None);
    assert_eq!(closed.emailed_by, None);
    let reopened = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + 3 * HOUR,
    )
    .await
    .unwrap();
    assert_eq!(reopened.waiting_on, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn assigning_an_emailed_case_ends_the_email_out() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    set_waiting_on(
        &store,
        WS,
        &id,
        WaitingOn::Contractor,
        Some("user:sam"),
        OPENED + HOUR,
    )
    .await
    .unwrap();
    assign(
        &store,
        WS,
        &id,
        Some("user:priya"),
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.assigned_to.as_deref(), Some("user:priya"));
    assert_eq!(case.waiting_on, None);
    assert_eq!(case.emailed_by, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reopening_to_open_returns_the_job_to_the_queue() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    assign(&store, WS, &id, Some("user:sam"), "user:sam", OPENED + HOUR)
        .await
        .unwrap();
    workflow_with_reason(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        Some("Fixed"),
        "user:sam",
        OPENED + 2 * HOUR,
    )
    .await
    .unwrap();
    let closed = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(
        closed.assigned_to.as_deref(),
        Some("user:sam"),
        "Close keeps the owner on record"
    );
    let reopened = workflow(
        &store,
        WS,
        &id,
        Workflow::ToAction,
        None,
        None,
        "user:sam",
        OPENED + 3 * HOUR,
    )
    .await
    .unwrap();
    assert_eq!(reopened.workflow, Workflow::ToAction);
    assert_eq!(reopened.assigned_to, None);
    assert_eq!(reopened.assigned_by, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unassigning_a_held_case_keeps_the_hold() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    assign(&store, WS, &id, Some("user:sam"), "user:sam", OPENED + HOUR)
        .await
        .unwrap();
    hold(&store, &id, OPENED + 2 * HOUR).await;
    assign(&store, WS, &id, None, "user:sam", OPENED + 3 * HOUR)
        .await
        .unwrap();
    let case = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(case.assigned_to, None);
    assert_eq!(case.snooze_until, Some(OPENED + 26 * HOUR));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn emailing_or_holding_unowned_work_makes_the_sender_its_owner() {
    let store = Store::memory().await.unwrap();
    let emailed = clocked(&store).await;
    set_waiting_on(
        &store,
        WS,
        &emailed,
        WaitingOn::Contractor,
        Some("user:sam"),
        OPENED + HOUR,
    )
    .await
    .unwrap();
    assert_eq!(
        get(&store, WS, &emailed)
            .await
            .unwrap()
            .unwrap()
            .assigned_to
            .as_deref(),
        Some("user:sam")
    );
    let store = Store::memory().await.unwrap();
    let held = clocked(&store).await;
    hold(&store, &held, OPENED + HOUR).await;
    assert_eq!(
        get(&store, WS, &held)
            .await
            .unwrap()
            .unwrap()
            .assigned_to
            .as_deref(),
        Some("user:sam")
    );
    // An owner already there stays; a system actor never takes ownership.
    let store = Store::memory().await.unwrap();
    let owned = clocked(&store).await;
    assign(
        &store,
        WS,
        &owned,
        Some("user:priya"),
        "user:priya",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    hold(&store, &owned, OPENED + 2 * HOUR).await;
    assert_eq!(
        get(&store, WS, &owned)
            .await
            .unwrap()
            .unwrap()
            .assigned_to
            .as_deref(),
        Some("user:priya")
    );
    let store = Store::memory().await.unwrap();
    let by_rule = clocked(&store).await;
    snooze(
        &store,
        WS,
        &by_rule,
        OPENED + 24 * HOUR,
        Some("auto"),
        "system:rule",
        OPENED + HOUR,
    )
    .await
    .unwrap();
    assert_eq!(
        get(&store, WS, &by_rule)
            .await
            .unwrap()
            .unwrap()
            .assigned_to,
        None
    );
}
