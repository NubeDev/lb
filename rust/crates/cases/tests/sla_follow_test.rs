//! The SLA clock, the closing reason and the personal pin (Insights review 2026-10). The clock runs
//! only while a case is Open: leaving Open (actioned, assigned, emailed, on hold, closed) stops it,
//! coming back adds the time spent out to the case, no breach is recorded while it is stopped, and
//! a breach already recorded stays. Against a real store; the deadline recompute itself is the host's.

use std::collections::BTreeSet;

use lb_cases::{
    assign, events, get, list, mark_breached, open, pin, set_deadlines, settle_sla_clocks,
    sla_state, snooze, workflow, workflow_with_reason, Deadlines, EventKind, Grouping, Lane,
    ListFilter, ListQuery, OpenInput, Resolution, SlaState, WaitingOn, Workflow,
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

/// Far past any deadline: a hold "until cleared".
const FOREVER: u64 = u64::MAX / 2;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn leaving_open_stops_the_clock_and_coming_back_adds_the_time_out() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;

    // Open: the clock runs and the case reads against its deadline.
    let fresh = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(fresh.sla_paused_at, None);
    assert_eq!(sla_state(&fresh, OPENED + HOUR, HOUR), SlaState::OnTrack);

    // Actioned one hour in: the clock stops, however late it gets, and no breach is recorded.
    let t1 = OPENED + HOUR;
    let actioned = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(actioned.sla_paused_at, Some(t1));
    let late = DUE + HOUR;
    assert_eq!(sla_state(&actioned, late, HOUR), SlaState::Paused);
    assert!(
        mark_breached(&store, WS, &id, late, "system:sla-clock", late)
            .await
            .unwrap()
            .is_none(),
        "a stopped clock does not breach"
    );

    // Reopened three hours later: three hours credited, the clock runs again, the history says so.
    let t2 = t1 + 3 * HOUR;
    let reopened = workflow(
        &store,
        WS,
        &id,
        Workflow::ToAction,
        None,
        None,
        "user:sam",
        t2,
    )
    .await
    .unwrap();
    assert_eq!(reopened.sla_paused_at, None);
    assert_eq!(reopened.sla_paused_ms, 3 * HOUR);
    let history = kinds(&store, &id).await;
    assert_eq!(
        history
            .iter()
            .filter(|k| **k == EventKind::SlaPaused)
            .count(),
        1,
        "{history:?}"
    );
    assert_eq!(
        history
            .iter()
            .filter(|k| **k == EventKind::SlaResumed)
            .count(),
        1,
        "{history:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_status_out_of_open_stops_the_clock() {
    let t1 = OPENED + HOUR;

    // An owner alone does not move a case out of Open (the status chip still says Open, so the clock
    // runs); assigned work in hand — Assigned — stops it.
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    assign(&store, WS, &id, Some("user:priya"), "user:sam", t1)
        .await
        .unwrap();
    let c = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(c.sla_paused_at, None, "owned but still Open");
    let c = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        t1 + 1,
    )
    .await
    .unwrap();
    assert_eq!(c.sla_paused_at, Some(t1 + 1), "assigned");

    // Waiting on someone (emailed / waiting on a client).
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    let c = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        Some(WaitingOn::Client),
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(c.sla_paused_at, Some(t1), "waiting on");

    // On hold, dated or "until cleared": both stop it.
    for until in [t1 + HOUR, FOREVER] {
        let store = Store::memory().await.unwrap();
        let id = clocked(&store).await;
        let c = snooze(
            &store,
            WS,
            &id,
            until,
            Some("parts on order"),
            "user:sam",
            t1,
        )
        .await
        .unwrap();
        assert_eq!(c.sla_paused_at, Some(t1), "hold until {until}");
        assert_eq!(sla_state(&c, DUE + HOUR, HOUR), SlaState::Paused);
    }

    // A case with no deadline has no clock to stop.
    let store = Store::memory().await.unwrap();
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
    let c = workflow(
        &store,
        WS,
        &bare,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        t1,
    )
    .await
    .unwrap();
    assert_eq!(c.sla_paused_at, None, "no deadline, nothing to pause");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_breach_while_open_is_history_and_a_close_without_one_is_met() {
    let store = Store::memory().await.unwrap();

    // Left Open past its deadline: breached, and it stays breached once actioned, closed or reopened.
    let id = clocked(&store).await;
    let late = DUE + HOUR;
    let open_late = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(
        sla_state(&open_late, late, HOUR),
        SlaState::Breached,
        "overdue while Open"
    );
    assert!(
        mark_breached(&store, WS, &id, late, "system:sla-clock", late)
            .await
            .unwrap()
            .is_some()
    );
    let actioned = workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        late + 1,
    )
    .await
    .unwrap();
    assert_eq!(sla_state(&actioned, late + 2, HOUR), SlaState::Breached);
    let closed = workflow(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:sam",
        late + 3,
    )
    .await
    .unwrap();
    assert_eq!(sla_state(&closed, late + 4, HOUR), SlaState::Breached);
    let reopened = workflow(
        &store,
        WS,
        &id,
        Workflow::ToAction,
        None,
        None,
        "user:sam",
        late + 5,
    )
    .await
    .unwrap();
    assert_eq!(
        sla_state(&reopened, late + 6, HOUR),
        SlaState::Breached,
        "a reopen keeps the breach"
    );

    // Actioned in time, then closed: Met, and the clock stays stopped for good.
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
    let closed = workflow(
        &store,
        WS,
        &id,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:sam",
        late,
    )
    .await
    .unwrap();
    assert_eq!(sla_state(&closed, late + HOUR, HOUR), SlaState::Met);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_settle_pass_pauses_cases_stored_before_the_rule_without_touching_their_activity() {
    let store = Store::memory().await.unwrap();
    let id = clocked(&store).await;
    let t1 = OPENED + HOUR;
    workflow(
        &store,
        WS,
        &id,
        Workflow::Actioned,
        None,
        None,
        "user:sam",
        t1,
    )
    .await
    .unwrap();

    // Make it look like a case written before the rule: out of Open, no pause on record.
    let mut old = get(&store, WS, &id).await.unwrap().unwrap();
    old.sla_paused_at = None;
    let value = serde_json::to_value(&old).unwrap();
    lb_store::write(&store, WS, "case", &id, &value)
        .await
        .unwrap();

    let now = DUE + 5 * HOUR;
    assert_eq!(settle_sla_clocks(&store, WS, now).await.unwrap(), 1);
    let settled = get(&store, WS, &id).await.unwrap().unwrap();
    assert_eq!(
        settled.sla_paused_at,
        Some(t1),
        "paused from when it last changed"
    );
    assert_eq!(
        settled.last_activity_ts, t1,
        "the platform keeping its books is not activity"
    );
    assert_eq!(
        settle_sla_clocks(&store, WS, now).await.unwrap(),
        0,
        "idempotent"
    );
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
