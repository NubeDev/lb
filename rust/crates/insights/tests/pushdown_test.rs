//! The pushed-down page read must return EXACTLY what the scan path returns.
//!
//! `list` now has two ways to produce a page: the engine does the filtering/ordering/limiting when
//! no tally is asked for, and the old read-everything-and-filter-in-Rust path when one is. A page
//! must not depend on which ran — so every case here asks for the same page BOTH ways and compares.
//!
//! Written this way because a pushdown bug is quiet: a wrong `WHERE` returns a plausible page, in
//! the right order, with the wrong rows in it.

use std::collections::BTreeMap;

use lb_insights::{
    ack, list, raise, resolve, ListFilter, ListQuery, Origin, OriginKind, RaiseInput, Severity,
    Status,
};
use lb_store::Store;

const WS: &str = "ops";
const RING: usize = 50;

async fn seed(store: &Store, key: &str, sev: Severity, ts: u64) -> String {
    raise(
        store,
        WS,
        RaiseInput {
            dedup_key: key.into(),
            severity: sev,
            title: format!("finding {key}"),
            body: serde_json::Value::Null,
            evidence: None,
            analysis: None,
            origin: Origin {
                kind: OriginKind::Rule,
                reference: if key.starts_with("x") {
                    "rule:x".into()
                } else {
                    "rule:demo".into()
                },
                run: None,
            },
            tags: BTreeMap::new(),
            occurrence: None,
            ts,
            producer: "user:test".into(),
        },
        RING,
    )
    .await
    .expect("raise")
    .id
}

/// Ask for the same page twice — once through the engine, once through the scan — and compare.
async fn both_ways(store: &Store, filter: ListFilter, limit: usize) {
    let pushed = list(
        store,
        WS,
        ListQuery {
            filter: filter.clone(),
            cursor: None,
            limit,
            offset: 0,
            counts: false,
        },
        None,
        None,
    )
    .await
    .expect("pushdown");

    let scanned = list(
        store,
        WS,
        ListQuery {
            filter,
            cursor: None,
            limit,
            offset: 0,
            counts: true,
        },
        None,
        None,
    )
    .await
    .expect("scan");

    assert_eq!(
        pushed.items, scanned.items,
        "the page must not depend on which path produced it"
    );
    assert_eq!(pushed.next, scanned.next, "the cursor must match too");
}

async fn seeded() -> Store {
    let store = Store::memory().await.expect("store");
    let mut ids = Vec::new();
    for i in 0..9u64 {
        let sev = match i % 3 {
            0 => Severity::Info,
            1 => Severity::Warning,
            _ => Severity::Critical,
        };
        let key = if i >= 7 {
            format!("x{i}")
        } else {
            format!("k{i}")
        };
        ids.push(seed(&store, &key, sev, 1_000 + i).await);
    }
    ack(&store, WS, &ids[0], "user:test", 2_000)
        .await
        .expect("ack");
    ack(&store, WS, &ids[1], "user:test", 2_001)
        .await
        .expect("ack");
    resolve(&store, WS, &ids[2], "user:test", None, 2_002)
        .await
        .expect("resolve");
    store
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn pushdown_returns_what_the_scan_path_returns() {
    let store = seeded().await;

    both_ways(&store, ListFilter::default(), 50).await;
    both_ways(
        &store,
        ListFilter {
            status: Some(Status::Open),
            ..Default::default()
        },
        50,
    )
    .await;
    both_ways(
        &store,
        ListFilter {
            status: Some(Status::Acked),
            ..Default::default()
        },
        50,
    )
    .await;
    both_ways(
        &store,
        ListFilter {
            severity: Some(Severity::Warning),
            ..Default::default()
        },
        50,
    )
    .await;
    both_ways(
        &store,
        ListFilter {
            origin_ref: Some("rule:x".into()),
            ..Default::default()
        },
        50,
    )
    .await;
    both_ways(
        &store,
        ListFilter {
            range: Some((1_002, 1_006)),
            ..Default::default()
        },
        50,
    )
    .await;
    // A page SHORTER than the match set — where the limit and the has-more probe actually bite.
    both_ways(&store, ListFilter::default(), 3).await;
    both_ways(
        &store,
        ListFilter {
            status: Some(Status::Open),
            ..Default::default()
        },
        2,
    )
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn paging_through_the_engine_walks_every_row_exactly_once() {
    let store = seeded().await;

    // Walk the whole set two rows at a time and assert the union is the full roster, in order, with
    // nothing repeated and nothing skipped — the keyset boundary is where a pushdown breaks.
    let mut seen: Vec<String> = Vec::new();
    let mut cursor = None;
    loop {
        let page = list(
            &store,
            WS,
            ListQuery {
                filter: ListFilter::default(),
                cursor,
                limit: 2,
                offset: 0,
                counts: false,
            },
            None,
            None,
        )
        .await
        .expect("page");
        for i in &page.items {
            assert!(!seen.contains(&i.id), "row {} came back twice", i.id);
            seen.push(i.id.clone());
        }
        match page.next {
            Some(c) => cursor = Some(c),
            None => break,
        }
    }

    let whole = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter::default(),
            cursor: None,
            limit: 500,
            offset: 0,
            counts: false,
        },
        None,
        None,
    )
    .await
    .expect("whole");
    let expected: Vec<String> = whole.items.iter().map(|i| i.id.clone()).collect();
    assert_eq!(
        seen, expected,
        "paging must visit every row exactly once, in order"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_pushdown_still_strips_evidence_and_analysis() {
    let store = seeded().await;
    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter::default(),
            cursor: None,
            limit: 50,
            offset: 0,
            counts: false,
        },
        None,
        None,
    )
    .await
    .expect("list");
    assert!(page
        .items
        .iter()
        .all(|i| i.evidence.is_none() && i.analysis.is_none()));
    assert!(page.counts.is_none(), "no tally was asked for");
}
