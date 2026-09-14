//! `count` must agree with `list`, always — over a REAL store, not a mock.
//!
//! The verb exists so the stat tiles stop downloading every row to tally them client-side. That
//! only holds if the two reads cannot disagree: a tile saying "78 open" beside a roster showing a
//! different set is worse than a slow tile, because nobody can tell which one is lying.
//!
//! So every case here asserts the SQL `GROUP BY` against the number `list` produces by filtering
//! the same data in Rust. The SQL is the thing under test — a `GROUP BY` that silently groups the
//! wrong idiom returns plausible integers, which is exactly the failure a hand-checked constant
//! would wave through.

use std::collections::{BTreeMap, HashSet};

use lb_insights::{
    ack, count, list, raise, resolve, AssigneeFilter, ListFilter, ListQuery, Origin, OriginKind,
    RaiseInput, Severity, Status,
};
use lb_store::Store;

const WS: &str = "ops";
const RING: usize = 50;

async fn seed(store: &Store, key: &str, severity: Severity, ts: u64) -> String {
    let out = raise(
        store,
        WS,
        RaiseInput {
            dedup_key: key.into(),
            severity,
            title: format!("finding {key}"),
            body: serde_json::Value::Null,
            evidence: None,
            analysis: None,
            origin: Origin {
                kind: OriginKind::Rule,
                reference: "rule:demo".into(),
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
    .expect("raise");
    out.id
}

/// How many rows `list` returns for a status — the Rust-side truth the SQL must match.
async fn listed(store: &Store, status: Option<Status>) -> usize {
    let query = ListQuery {
        filter: ListFilter {
            status,
            ..ListFilter::default()
        },
        cursor: None,
        limit: 500,
        offset: 0,
        counts: false,
    };
    list(store, WS, query, None, None)
        .await
        .expect("list")
        .items
        .len()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_tally_matches_what_list_returns_for_each_status() {
    let store = Store::memory().await.expect("store");

    // 6 findings → 3 left open, 2 acked, 1 resolved.
    let mut ids = Vec::new();
    for i in 0..6u64 {
        ids.push(seed(&store, &format!("k{i}"), Severity::Warning, 1_000 + i).await);
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

    let c = count(&store, WS, &ListFilter::default(), None, None)
        .await
        .expect("count");

    assert_eq!(c.total, 6, "every row counts toward the total");
    assert_eq!(c.acked, 2);
    assert_eq!(c.resolved, 1);
    assert_eq!(c.open, 3);

    // The property that matters: the tile and the roster cannot disagree.
    assert_eq!(c.open as usize, listed(&store, Some(Status::Open)).await);
    assert_eq!(c.acked as usize, listed(&store, Some(Status::Acked)).await);
    assert_eq!(
        c.resolved as usize,
        listed(&store, Some(Status::Resolved)).await
    );
    assert_eq!(c.total as usize, listed(&store, None).await);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_table_counts_zero_rather_than_erroring() {
    let store = Store::memory().await.expect("store");
    let c = count(&store, WS, &ListFilter::default(), None, None)
        .await
        .expect("count on an empty table");
    assert_eq!(c, Default::default());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_empty_tag_allowlist_counts_nothing_not_everything() {
    let store = Store::memory().await.expect("store");
    for i in 0..3u64 {
        seed(&store, &format!("t{i}"), Severity::Info, 1_000 + i).await;
    }

    // The host resolved a tag facet that matched no entity. Counting everything here is the bug
    // this asserts against — a tile would read "3 total" for a facet with no members.
    let empty: HashSet<String> = HashSet::new();
    let c = count(&store, WS, &ListFilter::default(), Some(&empty), None)
        .await
        .expect("count");
    assert_eq!(c.total, 0, "an empty allowlist matches nothing");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_severity_floor_matches_lists_at_least_semantics() {
    let store = Store::memory().await.expect("store");
    seed(&store, "s-info", Severity::Info, 1_000).await;
    seed(&store, "s-warn", Severity::Warning, 1_001).await;
    seed(&store, "s-crit", Severity::Critical, 1_002).await;

    let filter = ListFilter {
        severity: Some(Severity::Warning),
        ..ListFilter::default()
    };
    let c = count(&store, WS, &filter, None, None).await.expect("count");

    // warning + critical, never info — the SQL set must be derived from the same rank `at_least`
    // uses, so this number cannot drift from the roster's.
    assert_eq!(c.total, 2);
    let page = list(
        &store,
        WS,
        ListQuery {
            filter,
            cursor: None,
            limit: 500,
            offset: 0,
            counts: false,
        },
        None,
        None,
    )
    .await
    .expect("list");
    assert_eq!(c.total as usize, page.items.len());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_unassigned_filter_counts_only_rows_with_no_owner() {
    let store = Store::memory().await.expect("store");
    for i in 0..3u64 {
        seed(&store, &format!("u{i}"), Severity::Info, 1_000 + i).await;
    }
    let c = count(
        &store,
        WS,
        &ListFilter::default(),
        None,
        Some(&AssigneeFilter::Unassigned),
    )
    .await
    .expect("count");
    assert_eq!(c.total, 3, "nothing is assigned yet");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_with_counts_returns_the_same_tally_as_the_sql_path() {
    let store = Store::memory().await.expect("store");
    let mut ids = Vec::new();
    for i in 0..5u64 {
        ids.push(seed(&store, &format!("c{i}"), Severity::Warning, 1_000 + i).await);
    }
    ack(&store, WS, &ids[0], "user:test", 2_000)
        .await
        .expect("ack");
    resolve(&store, WS, &ids[1], "user:test", None, 2_001)
        .await
        .expect("resolve");

    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter::default(),
            cursor: None,
            limit: 500,
            offset: 0,
            counts: true,
        },
        None,
        None,
    )
    .await
    .expect("list");

    let standalone = count(&store, WS, &ListFilter::default(), None, None)
        .await
        .expect("count");
    assert_eq!(
        page.counts,
        Some(standalone),
        "one call or two, the tally is identical"
    );
    assert_eq!(page.items.len(), 5);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn counts_are_absent_unless_asked_for() {
    let store = Store::memory().await.expect("store");
    seed(&store, "q0", Severity::Info, 1_000).await;
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
    assert!(
        page.counts.is_none(),
        "absent, not zero — the reader must be able to tell"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_status_filtered_page_still_reports_all_four_numbers() {
    let store = Store::memory().await.expect("store");
    let mut ids = Vec::new();
    for i in 0..4u64 {
        ids.push(seed(&store, &format!("f{i}"), Severity::Info, 1_000 + i).await);
    }
    resolve(&store, WS, &ids[0], "user:test", None, 2_000)
        .await
        .expect("resolve");

    // The page narrows to `open`; the tally must NOT — the tiles beside it show every state.
    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter {
                status: Some(Status::Open),
                ..ListFilter::default()
            },
            cursor: None,
            limit: 50,
            offset: 0,
            counts: true,
        },
        None,
        None,
    )
    .await
    .expect("list");

    assert_eq!(page.items.len(), 3, "the page is filtered");
    let c = page.counts.expect("counts requested");
    assert_eq!(c.total, 4, "the tally is not");
    assert_eq!(c.open, 3);
    assert_eq!(c.resolved, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn limit_zero_with_counts_returns_the_tally_and_no_rows() {
    let store = Store::memory().await.expect("store");
    let mut ids = Vec::new();
    for i in 0..4u64 {
        ids.push(seed(&store, &format!("z{i}"), Severity::Warning, 1_000 + i).await);
    }
    ack(&store, WS, &ids[0], "user:test", 2_000)
        .await
        .expect("ack");

    // The counter-tile call: one integer per state, zero records on the wire.
    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter::default(),
            cursor: None,
            limit: 0,
            offset: 0,
            counts: true,
        },
        None,
        None,
    )
    .await
    .expect("list");

    assert!(page.items.is_empty(), "limit 0 fetches no rows at all");
    assert!(page.next.is_none());
    let c = page.counts.expect("counts requested");
    assert_eq!(c.total, 4);
    assert_eq!(c.acked, 1);
    assert_eq!(c.open, 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_tallied_page_still_honours_the_offset() {
    let store = Store::memory().await.expect("store");

    // 6 findings with rising timestamps. The roster is newest-first, so k5 leads and k0 trails.
    for i in 0..6u64 {
        seed(&store, &format!("k{i}"), Severity::Warning, 1_000 + i).await;
    }

    async fn page_at(store: &Store, offset: usize) -> lb_insights::ListPage {
        list(
            store,
            WS,
            ListQuery {
                filter: ListFilter::default(),
                cursor: None,
                limit: 2,
                offset,
                counts: true,
            },
            None,
            None,
        )
        .await
        .expect("list")
    }

    let titles = |p: &lb_insights::ListPage| {
        p.items
            .iter()
            .map(|i| i.title.clone())
            .collect::<Vec<String>>()
    };

    // THE BUG THIS PINS: asking for a tally sent the read down the path that sees every matching row
    // (it must, to break the set down by status) — and that path never consulted `offset`. So a
    // caller asking for page 2 was handed page 1, with nothing in the reply admitting it. A wrong
    // page returned confidently is worse than a refusal.
    assert_eq!(
        titles(&page_at(&store, 0).await),
        ["finding k5", "finding k4"]
    );
    assert_eq!(
        titles(&page_at(&store, 2).await),
        ["finding k3", "finding k2"]
    );
    assert_eq!(
        titles(&page_at(&store, 4).await),
        ["finding k1", "finding k0"]
    );

    // The tally describes the whole matching set, not the page in hand, so it must not shrink as the
    // caller pages through.
    for offset in [0usize, 2, 4] {
        let page = page_at(&store, offset).await;
        assert_eq!(
            page.counts.as_ref().expect("counts were asked for").total,
            6,
            "the tally is of the SET, not of the page at offset {offset}"
        );
    }

    // Past the end is empty — never a wrapped first page.
    assert!(
        page_at(&store, 6).await.items.is_empty(),
        "an offset past the end returns nothing"
    );
}
