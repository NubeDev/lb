//! The SLA pause, the closing reason and the personal pin (Insights review 2026-10). The pause: a snooze with an end date stops the clock, its end adds
//! the paused time to the case, an open-ended snooze pauses nothing, and no breach is recorded while
//! the clock is stopped. Against a real store; the deadline recompute itself is the host's.

use std::collections::BTreeSet;

use lb_cases::{
    events, get, list, mark_breached, open, pin, set_deadlines, sla_state, snooze, workflow,
    workflow_with_reason, Deadlines, EventKind, Grouping, Lane, ListFilter, ListQuery, OpenInput,
    Resolution, SlaState, Workflow, MAX_SLA_PAUSE_MS,
};
use lb_store::Store;

const WS: &str = "ops";
const HOUR: u64 = 3_600_000;
const OPENED: u64 = 1_000;
/// Due eight hours after opening.
const DUE: u64 = OPENED + 8 * HOUR;

/// A case with a deadline from a policy the store does not hold, so its pauses are measured in wall
/// time (the fallback for a policy since deleted) and the arithmetic in these tests stays plain.
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

async fn kinds(store: &Store, id: &str) -> Vec<EventKind> {
    let page = events(store, WS, id, 50, None).await.unwrap();
    page.items.into_iter().map(|e| e.kind).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_snooze_with_an_end_pauses_and_its_end_adds_the_paused_time() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;

    let t1 = OPENED + HOUR;
    let paused = snooze(
        &store,
        WS,
        &id,
        t1 + 4 * HOUR,
        Some("tenant away"),
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(paused.sla_paused_at, Some(t1));

    // While paused the case reads as it stood at the pause, however late it gets, and the breach
    // alarm for the old deadline records nothing.
    let late = DUE + HOUR;
    assert_eq!(sla_state(&paused, late, HOUR), SlaState::OnTrack);
    assert!(
        mark_breached(&store, WS, &id, late, "system:sla-clock", late)
            .await
            .unwrap()
            .is_none()
    );

    // Changing the snooze's end does not restart the pause.
    let t2 = t1 + HOUR;
    let moved = snooze(
        &store,
        WS,
        &id,
        t2 + 6 * HOUR,
        Some("still away"),
        "user:sam",
        t2,
    )
    .await
    .unwrap();
    assert_eq!(moved.sla_paused_at, Some(t1));

    // Un-snoozed three hours in: three hours paused, and the history says so.
    let t3 = t1 + 3 * HOUR;
    let resumed = snooze(&store, WS, &id, t3, None, "user:sam", t3)
        .await
        .unwrap();
    assert_eq!(resumed.sla_paused_at, None);
    assert_eq!(resumed.sla_paused_ms, 3 * HOUR);
    let history = kinds(&store, &id).await;
    assert!(history.contains(&EventKind::SlaPaused), "{history:?}");
    assert_eq!(
        history
            .iter()
            .filter(|k| **k == EventKind::SlaResumed)
            .count(),
        1
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_open_ended_snooze_pauses_nothing_and_ending_one_resumes() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;

    let t1 = OPENED + HOUR;
    // Open-ended is measured from when the snooze is set.
    let forever = |from: u64| from + MAX_SLA_PAUSE_MS + 1;
    let open_ended = snooze(
        &store,
        WS,
        &id,
        forever(t1),
        Some("someday"),
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(
        open_ended.sla_paused_at, None,
        "an open-ended snooze keeps the clock running"
    );

    // A dated snooze pauses; turning it open-ended ends the pause.
    let t2 = t1 + HOUR;
    snooze(
        &store,
        WS,
        &id,
        t2 + HOUR,
        Some("back at 3"),
        "user:sam",
        t2,
    )
    .await
    .unwrap();
    let t3 = t2 + HOUR / 2;
    let turned = snooze(
        &store,
        WS,
        &id,
        forever(t3),
        Some("someday"),
        "user:sam",
        t3,
    )
    .await
    .unwrap();
    assert_eq!(turned.sla_paused_at, None);
    assert_eq!(turned.sla_paused_ms, HOUR / 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn waiting_on_someone_and_a_case_with_no_deadline_never_pause() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    let t1 = OPENED + HOUR;
    let waiting = workflow(
        &store,
        WS,
        &id,
        Workflow::WaitingOnPo,
        None,
        None,
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(waiting.sla_paused_at, None);

    let input = OpenInput {
        title: "no contract".into(),
        grouping: Grouping::Single,
        primary_insight: "ins-2".into(),
        severity: "info".into(),
        ..Default::default()
    };
    let bare = open(&store, WS, input, "user:test", OPENED)
        .await
        .unwrap()
        .id;
    let snoozed = snooze(&store, WS, &bare, t1 + HOUR, Some("later"), "user:sam", t1)
        .await
        .unwrap();
    assert_eq!(snoozed.sla_paused_at, None, "no deadline, nothing to pause");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_closing_reason_rides_only_the_resolve_and_a_reopen_clears_it() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;

    let refused = workflow_with_reason(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        Some("Fixed remotely"),
        "user:sam",
        2_000,
    )
    .await;
    assert!(
        refused.is_err(),
        "a reason on an open transition is two fields that disagree"
    );

    let closed = workflow_with_reason(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        Some("  Fixed remotely "),
        "user:sam",
        3_000,
    )
    .await
    .unwrap();
    assert_eq!(closed.resolution_reason.as_deref(), Some("Fixed remotely"));
    let page = events(&store, WS, &id, 1, None).await.unwrap();
    assert_eq!(page.items[0].data["reason"], "Fixed remotely");

    let reopened = workflow(
        &store,
        WS,
        &id,
        Workflow::ToAction,
        None,
        None,
        "user:sam",
        4_000,
    )
    .await
    .unwrap();
    assert_eq!(reopened.resolution_reason, None);
    assert_eq!(
        get(&store, WS, &id)
            .await
            .unwrap()
            .unwrap()
            .resolution_reason,
        None
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_queue_filters_on_the_callers_own_pins() {
    let store = Store::memory().await.unwrap();
    let pinned = clocked(&store).await;
    let input = OpenInput {
        title: "other".into(),
        grouping: Grouping::Single,
        primary_insight: "ins-3".into(),
        severity: "info".into(),
        ..Default::default()
    };
    let other = open(&store, WS, input, "user:test", OPENED)
        .await
        .unwrap()
        .id;
    pin(&store, WS, &pinned, true, "user:priya").await.unwrap();

    for (want, expect) in [(true, &pinned), (false, &other)] {
        let query = ListQuery {
            lane: Lane::Watching,
            filter: ListFilter {
                pinned: Some(want),
                pinned_for: Some("user:priya".into()),
                ..Default::default()
            },
            now: 5_000,
            include_closed: false,
            limit: 50,
        };
        let page = list(&store, WS, &query, &BTreeSet::new()).await.unwrap();
        let ids: Vec<_> = page.items.iter().map(|r| r.case.id.clone()).collect();
        assert_eq!(ids, vec![expect.clone()], "pinned = {want}");
    }

    // Somebody else's view: priya's pin is not theirs.
    let query = ListQuery {
        lane: Lane::Watching,
        filter: ListFilter {
            pinned: Some(true),
            pinned_for: Some("user:sam".into()),
            ..Default::default()
        },
        now: 5_000,
        include_closed: false,
        limit: 50,
    };
    assert!(list(&store, WS, &query, &BTreeSet::new())
        .await
        .unwrap()
        .items
        .is_empty());
}
