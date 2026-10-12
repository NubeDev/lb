//! `stages` — the unfiltered read (the roster's default) projects `id` + `workflow` in one statement
//! instead of decoding every case. What must hold: it answers exactly what the full read answers, so
//! a page open counts the same stages whichever path ran.

use std::collections::BTreeSet;

use lb_cases::{open, stages, workflow, Grouping, ListFilter, OpenInput, Resolution, Workflow};
use lb_store::Store;

const WS: &str = "ops";

async fn case(store: &Store, n: u32, site: &str) -> String {
    open(
        store,
        WS,
        OpenInput {
            title: format!("case {n}"),
            grouping: Grouping::Single,
            primary_insight: format!("ins-{n}"),
            severity: "warning".into(),
            site: Some(site.into()),
            ..Default::default()
        },
        "user:test",
        1_000 + u64::from(n),
    )
    .await
    .expect("open")
    .id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_unfiltered_read_matches_the_full_read() {
    let store = Store::memory().await.expect("mem store");
    let a = case(&store, 1, "north").await;
    let b = case(&store, 2, "north").await;
    let _c = case(&store, 3, "south").await;
    workflow(
        &store,
        WS,
        &a,
        Workflow::Actioned,
        None,
        None,
        "user:test",
        2_000,
    )
    .await
    .expect("actioned");
    workflow(
        &store,
        WS,
        &b,
        Workflow::Resolved,
        Some(Resolution::Fixed),
        None,
        "user:test",
        2_001,
    )
    .await
    .expect("resolved");

    // The projected path: no axis set.
    let fast: BTreeSet<(String, String)> = stages(&store, WS, &ListFilter::default(), 3_000)
        .await
        .expect("stages")
        .into_iter()
        .map(|s| {
            assert!(s.matches, "an empty filter matches every case");
            (s.id, format!("{:?}", s.workflow))
        })
        .collect();

    // The full path: a filter every case passes, so `matches_filter` decodes each one.
    let every_site = ListFilter {
        sites_allowed: Some(["north".to_string(), "south".to_string()].into()),
        ..Default::default()
    };
    let full: BTreeSet<(String, String)> = stages(&store, WS, &every_site, 3_000)
        .await
        .expect("stages")
        .into_iter()
        .map(|s| (s.id, format!("{:?}", s.workflow)))
        .collect();

    assert_eq!(fast.len(), 3, "closed cases included");
    assert_eq!(fast, full);
    assert!(fast.contains(&(a, "Actioned".into())));
    assert!(fast.contains(&(b, "Resolved".into())));
}
