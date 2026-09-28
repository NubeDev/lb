//! The column search over a real store, through the real `list` verb, on both read paths (the
//! pushed-down page and the counted scan).
//!
//! What a search means: the typed text appears ANYWHERE in a configured column's value as the cell
//! shows it, case ignored. The cases below are the ones that failed on a live store with the earlier
//! prefix-only index: the middle of a word, the end of a word, a one-character word ("Lot 3"), and a
//! row matching on a title the Name cell does not show.

use std::collections::BTreeMap;

use lb_insights::{
    ensure_search_indexes, list, raise, set_case_id, set_tags_echo, CaseScope, ListFilter,
    ListQuery, Origin, OriginKind, RaiseInput, Severity,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, title: &str, sev: Severity, tags: &[(&str, &str)]) -> String {
    let id = raise(
        store,
        WS,
        RaiseInput {
            dedup_key: title.into(),
            severity: sev,
            title: title.into(),
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
            ts: 1_788_000_000_000,
            producer: "user:test".into(),
        },
        50,
    )
    .await
    .expect("raise")
    .id;
    let echo: BTreeMap<String, String> = tags
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    set_tags_echo(store, WS, &id, &echo).await.expect("echo");
    id
}

/// The columns rubix-ai configures: each cell's fallback chain.
fn columns() -> Vec<Vec<String>> {
    [
        vec!["tag:short_name", "tag:insight", "title"],
        vec!["tag:site", "tag:building"],
        vec!["tag:state", "tag:region"],
        vec!["tag:subsystem", "tag:measurement"],
    ]
    .into_iter()
    .map(|c| c.into_iter().map(String::from).collect())
    .collect()
}

/// Titles differ from the Name cell on purpose: `insight` is what the Name column shows.
async fn seeded() -> Store {
    let store = Store::memory().await.expect("mem store");
    seed(
        &store,
        "t1 energy spike",
        Severity::Warning,
        &[
            ("insight", "High Water Usage"),
            ("site", "10 Produce Lane - Pooraka"),
            ("state", "SA"),
            ("measurement", "Water"),
        ],
    )
    .await;
    seed(
        &store,
        "t2",
        Severity::Critical,
        &[
            ("insight", "Water Meter Flatline"),
            ("site", "Lot 3 Sergeants Estate - Eastern Creek"),
            ("region", "NSW"),
            ("subsystem", "Hydraulics"),
        ],
    )
    .await;
    seed(
        &store,
        "t3",
        Severity::Warning,
        &[
            ("insight", "High Electrical Usage"),
            ("site", "Lot 13 Sergeants Estate - Eastern Creek"),
            ("state", "NSW"),
            ("measurement", "Energy"),
            ("meter", "MDB-7"),
        ],
    )
    .await;
    // No site tag: the Site cell falls back to `building`.
    seed(
        &store,
        "Chiller trip",
        Severity::Warning,
        &[("building", "Berrinba Depot")],
    )
    .await;
    ensure_search_indexes(&store, WS, &columns())
        .await
        .expect("indexes");
    store
}

fn query(search: &str, filter: ListFilter, counts: bool) -> ListQuery {
    ListQuery {
        filter: ListFilter {
            search: Some(search.into()),
            search_columns: columns(),
            ..filter
        },
        cursor: None,
        offset: 0,
        limit: 50,
        counts,
        sort: None,
        tag_counts: None,
    }
}

async fn titles(store: &Store, q: ListQuery) -> Vec<String> {
    let mut t: Vec<String> = list(store, WS, q, None, None)
        .await
        .expect("search")
        .items
        .into_iter()
        .map(|i| i.title)
        .collect();
    t.sort();
    t
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn text_anywhere_in_a_shown_value_matches_on_both_read_paths() {
    let store = seeded().await;
    let cases: &[(&str, &[&str], &str)] = &[
        ("pooraka", &["t1 energy spike"], "a whole word, any case"),
        ("raka", &["t1 energy spike"], "the MIDDLE of a word"),
        ("line", &["t2"], "the END of a word (Flatline)"),
        ("Lot 3", &["t2"], "a one-character word, and not 'Lot 13'"),
        (
            "produce lane",
            &["t1 energy spike"],
            "several words in order",
        ),
        ("nsw", &["t2", "t3"], "State: `state`, else `region`"),
        ("hydra", &["t2"], "Subsystem"),
        (
            "energy",
            &["t3"],
            "Subsystem; NOT the hidden title 't1 energy spike'",
        ),
        ("berrin", &["Chiller trip"], "Site falls back to `building`"),
        ("chiller", &["Chiller trip"], "Name falls back to the title"),
        ("mdb", &[], "a tag no column shows is not searched"),
        ("zzzz", &[], "no match"),
    ];
    for counts in [false, true] {
        for (term, want, why) in cases {
            let got = titles(&store, query(term, ListFilter::default(), counts)).await;
            assert_eq!(got, *want, "{term:?}: {why} (counts={counts})");
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_count_is_the_matching_rows_not_the_candidates() {
    let store = seeded().await;
    let page = list(
        &store,
        WS,
        query("lot 3", ListFilter::default(), true),
        None,
        None,
    )
    .await
    .expect("search");
    assert_eq!(page.items.len(), 1);
    assert_eq!(
        page.counts.expect("tally").total,
        1,
        "Lot 13 is a candidate, not a match"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_search_is_applied_inside_the_filters_not_over_everything() {
    let store = seeded().await;
    // "sergeants" alone matches t2 and t3; only t2 is critical and only t3's case is actioned.
    assert_eq!(
        titles(&store, query("sergeants", ListFilter::default(), false)).await,
        ["t2", "t3"]
    );
    let critical = ListFilter {
        severity: Some(Severity::Critical),
        ..Default::default()
    };
    for counts in [false, true] {
        assert_eq!(
            titles(&store, query("sergeants", critical.clone(), counts)).await,
            ["t2"]
        );
    }
    let t3 = list(
        &store,
        WS,
        query("electrical", ListFilter::default(), false),
        None,
        None,
    )
    .await
    .expect("find")
    .items[0]
        .id
        .clone();
    set_case_id(&store, WS, &t3, Some("case:A"))
        .await
        .expect("echo");
    let actioned = ListFilter {
        case: Some(CaseScope {
            stage_of: [("case:A".to_string(), "actioned".to_string())].into(),
            stages: Some(["actioned".to_string()].into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    for counts in [false, true] {
        let page = list(
            &store,
            WS,
            query("sergeants", actioned.clone(), counts),
            None,
            None,
        )
        .await
        .expect("search within a stage");
        let got: Vec<_> = page.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(got, ["t3"], "counts={counts}");
        if counts {
            assert_eq!(page.counts.expect("tally").total, 1);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn with_no_columns_configured_the_search_is_the_title_alone() {
    let store = seeded().await;
    let title_only = |term: &str| ListQuery {
        filter: ListFilter {
            search: Some(term.into()),
            ..Default::default()
        },
        cursor: None,
        offset: 0,
        limit: 50,
        counts: false,
        sort: None,
        tag_counts: None,
    };
    assert_eq!(
        titles(&store, title_only("spike")).await,
        ["t1 energy spike"]
    );
    assert!(
        titles(&store, title_only("pooraka")).await.is_empty(),
        "tags are not searched"
    );
}

/// Candidates come from the indexes, never a table scan, even with a filter beside the search. The
/// search runs as an inner query (its matches alone use the indexes); the filter and the exact check
/// run over its results. Joined into ONE `WHERE`, the planner scanned the whole table instead.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_candidates_come_from_the_indexes_even_beside_a_filter() {
    let store = seeded().await;
    let explain = |sql: &str| {
        let store = store.clone();
        let sql = format!("{sql} EXPLAIN");
        async move {
            let mut r = store.query_ws(WS, &sql, vec![]).await.expect("explain");
            let rows: Vec<serde_json::Value> = r.take(0).expect("decode");
            serde_json::to_string(&rows).unwrap()
        }
    };
    let inner = "SELECT * FROM insight WHERE data.title @0@ 'raka' OR data.tags.site @1@ 'raka'";
    let plan = explain(inner).await;
    println!("inner plan: {plan}");
    for index in ["insight_name_v3", "insight_tag_site_v3"] {
        assert!(
            plan.contains(index),
            "{index} must answer its branch: {plan}"
        );
    }
    let outer = format!(
        "SELECT data FROM ({inner}) WHERE data.severity IN ['warning'] \
         AND string::contains(string::lowercase(data.tags.site ?? ''), 'raka')"
    );
    let plan = explain(&outer).await;
    println!("outer plan: {}", &plan[..plan.len().min(300)]);
    assert!(
        !plan.contains("Iterate Table"),
        "the outer query must not scan the table: {plan}"
    );
}

/// The index build works on a store UPGRADED from the old analyzer: rows exist, but nothing has been
/// raised since, so the v3 analyzer the tag indexes name is not defined yet. It must define it, not
/// rely on a raise having done it (found live: "The analyzer 'insight_text_v3' does not exist").
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_index_build_works_on_an_upgraded_store_before_any_raise() {
    let store = Store::memory().await.expect("mem store");
    seed(
        &store,
        "t1",
        Severity::Warning,
        &[("site", "Lot 1 Sergeants Estate")],
    )
    .await;
    // Back to the pre-upgrade state: the rows stay, the v3 analyzer and its index go.
    store
        .query_ws(
            WS,
            "REMOVE INDEX IF EXISTS insight_name_v3 ON TABLE insight; \
             REMOVE ANALYZER IF EXISTS insight_text_v3;",
            vec![],
        )
        .await
        .expect("downgrade");
    ensure_search_indexes(&store, WS, &columns())
        .await
        .expect("builds on a store with rows and no v3 analyzer");
    assert_eq!(
        titles(&store, query("lot 1", ListFilter::default(), true)).await,
        ["t1"]
    );
}
