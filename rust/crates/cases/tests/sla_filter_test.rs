//! The `sla` filter axis — one reading of a case against its deadline, shared by the queue (`list`)
//! and the insight roster's case lens (`stages`). What must hold: each state is disjoint, the clock
//! is the caller's, and a closed case never becomes a breach after the fact.

use std::collections::BTreeSet;

use lb_cases::{
    list, mark_breached, open, set_deadlines, stages, workflow, Deadlines, Grouping, Lane,
    ListFilter, ListQuery, OpenInput, Resolution, SlaState, Workflow,
};
use lb_store::Store;

const WS: &str = "ops";
const HOUR: u64 = 3_600_000;
const OPENED: u64 = 1_000;

async fn case(store: &Store, n: u32, due_in_h: Option<u64>) -> String {
    let id = open(
        store,
        WS,
        OpenInput {
            title: format!("case {n}"),
            grouping: Grouping::Single,
            primary_insight: format!("ins-{n}"),
            severity: "warning".into(),
            ..Default::default()
        },
        "user:test",
        OPENED,
    )
    .await
    .expect("open")
    .id;
    if let Some(h) = due_in_h {
        set_deadlines(
            store,
            WS,
            &id,
            Some("p1"),
            Deadlines {
                respond_by: None,
                due_at: Some(OPENED + h * HOUR),
            },
            "system:sla-clock",
            OPENED,
        )
        .await
        .expect("deadlines");
    }
    id
}

async fn queue(store: &Store, sla: SlaState, now: u64, due_soon_ms: Option<u64>) -> Vec<String> {
    let query = ListQuery {
        lane: Lane::Watching,
        filter: ListFilter {
            sla: Some(sla),
            due_soon_ms,
            ..Default::default()
        },
        now,
        include_closed: false,
        limit: 50,
    };
    let mut ids: Vec<String> = list(store, WS, &query, &BTreeSet::new())
        .await
        .expect("list")
        .items
        .into_iter()
        .map(|r| r.case.id)
        .collect();
    ids.sort();
    ids
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_state_names_its_own_cases_at_the_callers_clock() {
    let store = Store::memory().await.expect("mem store");
    let overdue = case(&store, 1, Some(1)).await;
    let soon = case(&store, 2, Some(5)).await;
    let later = case(&store, 3, Some(48)).await;
    let unclocked = case(&store, 4, None).await;
    // Two hours in: the 1h case is past due, the 5h case is 3h off, the 48h case is far off.
    let now = OPENED + 2 * HOUR;

    assert_eq!(
        queue(&store, SlaState::Breached, now, None).await,
        [overdue]
    );
    assert_eq!(
        queue(&store, SlaState::DueSoon, now, None).await,
        std::slice::from_ref(&soon)
    );
    assert_eq!(
        queue(&store, SlaState::OnTrack, now, None).await,
        std::slice::from_ref(&later)
    );
    assert_eq!(
        queue(&store, SlaState::NoDeadline, now, None).await,
        [unclocked]
    );

    // The window is the caller's: at one hour the 5h case is on track, at two days nothing is.
    let mut on_track = [soon.clone(), later.clone()];
    on_track.sort();
    assert_eq!(
        queue(&store, SlaState::OnTrack, now, Some(HOUR)).await,
        on_track
    );
    let mut due_soon = [soon, later];
    due_soon.sort();
    assert_eq!(
        queue(&store, SlaState::DueSoon, now, Some(48 * HOUR)).await,
        due_soon
    );

    // Before the first deadline nothing is breached: the clock is the query's, not the wall's.
    assert!(queue(&store, SlaState::Breached, OPENED, None)
        .await
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_closed_case_is_met_unless_its_breach_was_recorded() {
    let store = Store::memory().await.expect("mem store");
    let in_time = case(&store, 1, Some(4)).await;
    let late = case(&store, 2, Some(1)).await;
    mark_breached(
        &store,
        WS,
        &late,
        OPENED + HOUR,
        "system:sla-clock",
        OPENED + 2 * HOUR,
    )
    .await
    .expect("breach");
    for id in [&in_time, &late] {
        workflow(
            &store,
            WS,
            id,
            Workflow::Resolved,
            Some(Resolution::Fixed),
            None,
            "user:test",
            OPENED + 3 * HOUR,
        )
        .await
        .expect("resolved");
    }

    // Long after both deadlines. The roster's lens reads closed cases too, so it is the read to ask.
    let now = OPENED + 100 * HOUR;
    let matching = |sla: SlaState| {
        let store = &store;
        async move {
            let filter = ListFilter {
                sla: Some(sla),
                ..Default::default()
            };
            let mut ids: Vec<String> = stages(store, WS, &filter, now)
                .await
                .expect("stages")
                .into_iter()
                .filter(|c| c.matches)
                .map(|c| c.id)
                .collect();
            ids.sort();
            ids
        }
    };
    assert_eq!(matching(SlaState::Met).await, [in_time]);
    assert_eq!(matching(SlaState::Breached).await, [late]);
    assert!(matching(SlaState::OnTrack).await.is_empty());
}
