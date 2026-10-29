//! The queue's Status, Owner and SLA-Paused filters (Insights review 2026-10): the words the status
//! chip reads — Open, Assigned, Emailed, On hold, Closed — are filter values the node answers, and a
//! held case reads Paused, never overdue. Against a real store.

use std::collections::BTreeSet;

use lb_cases::{
    assign, case_status, get, list, open, set_deadlines, set_waiting_on, snooze, workflow,
    CaseStatus, Deadlines, Grouping, Lane, ListFilter, ListQuery, OpenInput, Resolution, SlaState,
    WaitingOn, Workflow,
};
use lb_store::Store;

const WS: &str = "ops";
const HOUR: u64 = 3_600_000;
const T: u64 = 1_000;

async fn case(store: &Store, n: u32) -> String {
    let input = OpenInput {
        title: format!("case {n}"),
        grouping: Grouping::Single,
        primary_insight: format!("ins-{n}"),
        severity: "warning".into(),
        ..Default::default()
    };
    open(store, WS, input, "user:test", T).await.unwrap().id
}

async fn ids(store: &Store, filter: ListFilter, now: u64) -> Vec<String> {
    let query = ListQuery {
        lane: Lane::Watching,
        filter,
        now,
        include_closed: true,
        limit: 50,
    };
    let mut out: Vec<String> = list(store, WS, &query, &BTreeSet::new())
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|r| r.case.id)
        .collect();
    out.sort();
    out
}

/// One case in each status.
async fn five(store: &Store) -> [String; 5] {
    let open_ = case(store, 1).await;
    let assigned = case(store, 2).await;
    workflow(
        store,
        WS,
        &assigned,
        Workflow::Actioned,
        None,
        None,
        "user:test",
        T + 1,
    )
    .await
    .unwrap();
    assign(store, WS, &assigned, Some("user:priya"), "user:test", T + 2)
        .await
        .unwrap();
    let emailed = case(store, 3).await;
    workflow(
        store,
        WS,
        &emailed,
        Workflow::Actioned,
        None,
        None,
        "user:test",
        T + 1,
    )
    .await
    .unwrap();
    set_waiting_on(
        store,
        WS,
        &emailed,
        WaitingOn::Contractor,
        Some("user:test"),
        T + 2,
    )
    .await
    .unwrap();
    let held = case(store, 4).await;
    snooze(
        store,
        WS,
        &held,
        T + 24 * HOUR,
        Some("parts"),
        "user:test",
        T + 2,
    )
    .await
    .unwrap();
    let closed = case(store, 5).await;
    workflow(
        store,
        WS,
        &closed,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:test",
        T + 2,
    )
    .await
    .unwrap();
    [open_, assigned, emailed, held, closed]
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_status_filters_to_its_own_case() {
    let store = Store::memory().await.unwrap();
    let [open_, assigned, emailed, held, closed] = five(&store).await;
    let now = T + HOUR;
    for (status, want) in [
        (CaseStatus::Open, &open_),
        (CaseStatus::Assigned, &assigned),
        (CaseStatus::Emailed, &emailed),
        (CaseStatus::Hold, &held),
        (CaseStatus::Closed, &closed),
    ] {
        let got = ids(
            &store,
            ListFilter {
                status: Some(status),
                ..Default::default()
            },
            now,
        )
        .await;
        assert_eq!(got, vec![want.clone()], "{status:?}");
        let c = get(&store, WS, want).await.unwrap().unwrap();
        assert_eq!(case_status(&c, now), status);
    }
    // A hold whose end has passed is over: the case reads by its other fields again.
    let later = T + 48 * HOUR;
    assert!(ids(
        &store,
        ListFilter {
            status: Some(CaseStatus::Hold),
            ..Default::default()
        },
        later
    )
    .await
    .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn owner_filters_by_holder_or_none() {
    let store = Store::memory().await.unwrap();
    let [open_, assigned, emailed, held, closed] = five(&store).await;
    let priya = ListFilter {
        owner: Some("user:priya".into()),
        ..Default::default()
    };
    assert_eq!(ids(&store, priya, T).await, vec![assigned.clone()]);
    let none = ListFilter {
        owner: Some("none".into()),
        ..Default::default()
    };
    // The emailed and held cases are their sender's: emailing or holding unowned work owns it.
    let mut want = vec![open_, closed];
    want.sort();
    assert_eq!(ids(&store, none, T).await, want);
    let test = ListFilter {
        owner: Some("user:test".into()),
        ..Default::default()
    };
    let mut want = vec![emailed, held];
    want.sort();
    assert_eq!(ids(&store, test, T).await, want);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_held_case_reads_paused_not_overdue() {
    let store = Store::memory().await.unwrap();
    let held = case(&store, 1).await;
    let late = case(&store, 2).await;
    for id in [&held, &late] {
        let d = Deadlines {
            respond_by: None,
            due_at: Some(T + HOUR),
        };
        set_deadlines(&store, WS, id, Some("p1"), d, "system:sla-clock", T)
            .await
            .unwrap();
    }
    snooze(
        &store,
        WS,
        &held,
        T + 48 * HOUR,
        Some("parts"),
        "user:test",
        T + 1,
    )
    .await
    .unwrap();
    let now = T + 2 * HOUR;
    let sla = |s| ListFilter {
        sla: Some(s),
        ..Default::default()
    };
    assert_eq!(ids(&store, sla(SlaState::Paused), now).await, vec![held]);
    assert_eq!(ids(&store, sla(SlaState::Breached), now).await, vec![late]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_hold_until_cleared_stops_the_clock_like_any_hold() {
    let store = Store::memory().await.unwrap();
    let id = case(&store, 1).await;
    let d = Deadlines {
        respond_by: None,
        due_at: Some(T + HOUR),
    };
    set_deadlines(&store, WS, &id, Some("p1"), d, "system:sla-clock", T)
        .await
        .unwrap();
    // "Until I clear it": an end no date reaches.
    snooze(
        &store,
        WS,
        &id,
        u64::MAX / 2,
        Some("parts"),
        "user:test",
        T + 1,
    )
    .await
    .unwrap();
    let now = T + 2 * HOUR;
    let sla = |s| ListFilter {
        sla: Some(s),
        ..Default::default()
    };
    // A hold is not Open, so the SLA clock stopped when it began (Insights review 2026-10-29), end
    // date or none: the case reads Paused, not overdue.
    assert_eq!(
        ids(&store, sla(SlaState::Paused), now).await,
        vec![id.clone()]
    );
    assert!(ids(&store, sla(SlaState::Breached), now).await.is_empty());
    let status = ListFilter {
        status: Some(CaseStatus::Hold),
        ..Default::default()
    };
    assert_eq!(ids(&store, status, now).await, vec![id]);
}
