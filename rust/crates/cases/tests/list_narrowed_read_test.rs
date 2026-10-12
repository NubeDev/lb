//! `list` — the lane read asks the engine for open cases only (unless closed ones are wanted), takes
//! an `ids` filter, and reads every shipped row's member count in one query. What must hold: the
//! lanes return exactly what they returned before (closed cases out by default, in with
//! `include_closed`), the counts are right per case, and `ids` narrows to those cases.

use std::collections::BTreeSet;

use lb_cases::{
    list, member_add, open, workflow, Grouping, Lane, ListFilter, ListQuery, MemberRole, OpenInput,
    Resolution, Workflow, MAX_CASE_PAGE,
};
use lb_store::Store;

const WS: &str = "ops";

async fn case(store: &Store, n: u32) -> String {
    open(
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
        1_000 + u64::from(n),
    )
    .await
    .expect("open")
    .id
}

fn query(filter: ListFilter, include_closed: bool) -> ListQuery {
    ListQuery {
        lane: Lane::Watching,
        filter,
        now: 5_000,
        include_closed,
        limit: 50,
    }
}

async fn ids_of(store: &Store, q: &ListQuery) -> BTreeSet<String> {
    list(store, WS, q, &BTreeSet::new())
        .await
        .expect("list")
        .items
        .into_iter()
        .map(|r| r.case.id)
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn closed_cases_stay_out_unless_asked_and_counts_are_per_case() {
    let store = Store::memory().await.expect("mem store");
    let open_a = case(&store, 1).await;
    let open_b = case(&store, 2).await;
    let shut = case(&store, 3).await;
    // `open_a` cites two more detections; the others only their primary.
    for (i, ins) in ["ins-1a", "ins-1b"].iter().enumerate() {
        member_add(
            &store,
            WS,
            &open_a,
            ins,
            MemberRole::Explained,
            "user:test",
            false,
            1_100 + i as u64,
        )
        .await
        .expect("member add");
    }
    workflow(
        &store,
        WS,
        &shut,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:test",
        2_000,
    )
    .await
    .expect("resolved");

    let live = ids_of(&store, &query(ListFilter::default(), false)).await;
    assert_eq!(
        live,
        [open_a.clone(), open_b.clone()].into(),
        "closed case out by default"
    );
    let all = ids_of(&store, &query(ListFilter::default(), true)).await;
    assert_eq!(
        all,
        [open_a.clone(), open_b.clone(), shut.clone()].into(),
        "in when asked"
    );

    let page = list(
        &store,
        WS,
        &query(ListFilter::default(), true),
        &BTreeSet::new(),
    )
    .await
    .expect("list");
    for row in page.items {
        let want = if row.case.id == open_a { 3 } else { 1 };
        assert_eq!(row.member_count, want, "member count of {}", row.case.id);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ids_narrows_to_those_cases_and_a_long_list_is_refused() {
    let store = Store::memory().await.expect("mem store");
    let a = case(&store, 1).await;
    let _b = case(&store, 2).await;
    let shut = case(&store, 3).await;
    workflow(
        &store,
        WS,
        &shut,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:test",
        2_000,
    )
    .await
    .expect("resolved");

    let wanted = ListFilter {
        ids: Some(vec![a.clone(), shut.clone(), "no-such-case".into()]),
        ..Default::default()
    };
    assert_eq!(
        ids_of(&store, &query(wanted.clone(), true)).await,
        [a.clone(), shut.clone()].into(),
        "only the named cases; an unknown id is simply absent"
    );
    assert_eq!(
        ids_of(&store, &query(wanted, false)).await,
        [a].into(),
        "`ids` and `include_closed` both apply"
    );

    let too_many = ListFilter {
        ids: Some((0..=MAX_CASE_PAGE).map(|i| format!("c{i}")).collect()),
        ..Default::default()
    };
    assert!(list(&store, WS, &query(too_many, true), &BTreeSet::new())
        .await
        .is_err());
}
