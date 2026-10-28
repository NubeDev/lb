//! My Insights' five personal lanes (Insights review #16): each case sits in at most one, by the
//! order `my_lane` documents, and "by me" is the recorded actor. Against a real store.

use std::collections::BTreeSet;

use lb_cases::{
    assign, list, open, set_waiting_on, snooze, workflow, Grouping, Lane, ListFilter, ListQuery,
    OpenInput, Resolution, WaitingOn, Workflow, CLOSED_LANE_MS,
};
use lb_store::Store;

const WS: &str = "ops";
const ME: &str = "user:priya";
const SAM: &str = "user:sam";
const T0: u64 = 1_000;
const DAY: u64 = 24 * 3_600_000;

async fn case(store: &Store, title: &str) -> String {
    let input = OpenInput {
        title: title.into(),
        grouping: Grouping::Single,
        primary_insight: format!("ins-{title}"),
        severity: "warning".into(),
        ..Default::default()
    };
    open(store, WS, input, "system:grouping", T0)
        .await
        .unwrap()
        .id
}

async fn lane(store: &Store, who: &str, lane: Lane, now: u64) -> Vec<String> {
    let me: BTreeSet<String> = [who.to_string()].into();
    let query = ListQuery {
        lane,
        filter: ListFilter::default(),
        now,
        include_closed: false,
        limit: 50,
    };
    let mut titles: Vec<String> = list(store, WS, &query, &me)
        .await
        .unwrap()
        .items
        .into_iter()
        .map(|r| r.case.title)
        .collect();
    titles.sort();
    titles
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_case_is_in_the_one_lane_its_state_and_actor_say() {
    let store = Store::memory().await.unwrap();
    let now = T0 + 10;

    // Mine to act on.
    let act = case(&store, "act").await;
    assign(&store, WS, &act, Some(ME), SAM, T0).await.unwrap();
    // Mine, but snoozed: On hold, not My action.
    let hold = case(&store, "hold").await;
    assign(&store, WS, &hold, Some(ME), ME, T0).await.unwrap();
    snooze(&store, WS, &hold, now + DAY, Some("parts"), ME, T0)
        .await
        .unwrap();
    // I gave it to Sam.
    let gave = case(&store, "gave").await;
    assign(&store, WS, &gave, Some(SAM), ME, T0).await.unwrap();
    // Mine, and I sent it to a contractor: Emailed, not My action.
    let sent = case(&store, "sent").await;
    assign(&store, WS, &sent, Some(ME), ME, T0).await.unwrap();
    set_waiting_on(&store, WS, &sent, WaitingOn::Contractor, Some(ME), T0)
        .await
        .unwrap();
    // I closed it.
    let shut = case(&store, "shut").await;
    workflow(
        &store,
        WS,
        &shut,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        ME,
        T0,
    )
    .await
    .unwrap();
    // Sam's, assigned by Sam: none of mine.
    let other = case(&store, "other").await;
    assign(&store, WS, &other, Some(SAM), SAM, T0)
        .await
        .unwrap();

    assert_eq!(lane(&store, ME, Lane::Action, now).await, ["act"]);
    assert_eq!(lane(&store, ME, Lane::Hold, now).await, ["hold"]);
    assert_eq!(lane(&store, ME, Lane::Assigned, now).await, ["gave"]);
    assert_eq!(lane(&store, ME, Lane::Emailed, now).await, ["sent"]);
    assert_eq!(lane(&store, ME, Lane::Closed, now).await, ["shut"]);

    // Sam's view: his own work, and the one he gave to me.
    assert_eq!(
        lane(&store, SAM, Lane::Action, now).await,
        ["gave", "other"]
    );
    assert_eq!(lane(&store, SAM, Lane::Assigned, now).await, ["act"]);

    // A week on, the closed case leaves the Closed lane.
    assert!(lane(&store, ME, Lane::Closed, T0 + CLOSED_LANE_MS + 1)
        .await
        .is_empty());
    // The snooze ends: the held case is back to act on.
    assert_eq!(
        lane(&store, ME, Lane::Action, now + DAY + 1).await,
        ["act", "hold"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unassigning_forgets_who_assigned() {
    let store = Store::memory().await.unwrap();
    let id = case(&store, "x").await;
    assign(&store, WS, &id, Some(SAM), ME, T0).await.unwrap();
    assign(&store, WS, &id, None, ME, T0 + 1).await.unwrap();
    assert!(lane(&store, ME, Lane::Assigned, T0 + 2).await.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn my_work_others_emailed_or_closed_stays_in_my_view() {
    let store = Store::memory().await.unwrap();
    let now = T0 + 10;
    // Mine, and Sam sent it to a contractor: Emailed for me (not My action), and for Sam.
    let sent = case(&store, "sent").await;
    assign(&store, WS, &sent, Some(ME), ME, T0).await.unwrap();
    set_waiting_on(&store, WS, &sent, WaitingOn::Contractor, Some(SAM), T0)
        .await
        .unwrap();
    // Mine, and Sam closed it: in my Closed lane, and his.
    let shut = case(&store, "shut").await;
    assign(&store, WS, &shut, Some(ME), ME, T0).await.unwrap();
    workflow(
        &store,
        WS,
        &shut,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        SAM,
        T0,
    )
    .await
    .unwrap();
    // Mine with a hand-set waiting_on (no email sent): still mine to act on.
    let told = case(&store, "told").await;
    assign(&store, WS, &told, Some(ME), ME, T0).await.unwrap();
    set_waiting_on(&store, WS, &told, WaitingOn::Client, None, T0)
        .await
        .unwrap();

    assert_eq!(lane(&store, ME, Lane::Emailed, now).await, ["sent"]);
    assert_eq!(lane(&store, ME, Lane::Action, now).await, ["told"]);
    assert_eq!(lane(&store, ME, Lane::Closed, now).await, ["shut"]);
    assert_eq!(lane(&store, SAM, Lane::Emailed, now).await, ["sent"]);
    assert_eq!(lane(&store, SAM, Lane::Closed, now).await, ["shut"]);
}
