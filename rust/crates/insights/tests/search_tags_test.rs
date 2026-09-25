//! The search box matches the configured tag keys beside the title — over a real store, through the
//! real `list` verb, on both read paths (the pushed-down page and the counted scan).
//!
//! The risk this guards is the engine's, not ours: a search is `title @@ q OR tags.k @@ q`, and
//! SurrealDB only answers `@@` from an index. If a branch of the `OR` had no index, or two branches
//! shared a match reference, the read would error or match nothing, and the roster would look empty.

use std::collections::BTreeMap;

use lb_insights::{
    list, raise, set_tags_echo, ListFilter, ListQuery, Origin, OriginKind, RaiseInput, Severity,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, key: &str, title: &str, tags: &[(&str, &str)]) -> String {
    let id = raise(
        store,
        WS,
        RaiseInput {
            dedup_key: key.into(),
            severity: Severity::Warning,
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
    // The echo is what `list` returns as the row's tags; the host writes it after a raise.
    let echo: BTreeMap<String, String> = tags
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    set_tags_echo(store, WS, &id, &echo).await.expect("echo");
    id
}

fn keys(ks: &[&str]) -> Vec<String> {
    ks.iter().map(|k| k.to_string()).collect()
}

async fn search(store: &Store, term: &str, tags: &[String], counts: bool) -> Vec<String> {
    let page = list(
        store,
        WS,
        ListQuery {
            filter: ListFilter {
                search: Some(term.into()),
                search_tags: tags.to_vec(),
                ..Default::default()
            },
            cursor: None,
            offset: 0,
            limit: 50,
            counts,
        },
        None,
        None,
    )
    .await
    .expect("search");
    let mut titles: Vec<String> = page.items.into_iter().map(|i| i.title).collect();
    titles.sort();
    titles
}

async fn seeded() -> Store {
    let store = Store::memory().await.expect("mem store");
    seed(
        &store,
        "k1",
        "High daily usage",
        &[
            ("site", "Chullora Depot"),
            ("state", "NSW"),
            ("meter", "MDB-1-4"),
        ],
    )
    .await;
    seed(
        &store,
        "k2",
        "Chiller flatline",
        &[
            ("site", "Eastern Creek"),
            ("subsystem", "HVAC"),
            ("state", "VIC"),
        ],
    )
    .await;
    seed(
        &store,
        "k3",
        "Door left open",
        &[("site", "Wetherill Park")],
    )
    .await;
    store
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_configured_tag_is_searched_by_prefix_on_both_read_paths() {
    let store = seeded().await;
    let tags = keys(&["site", "state", "subsystem"]);
    for counts in [false, true] {
        // A tag prefix finds the row whose TITLE says nothing about it.
        assert_eq!(
            search(&store, "chul", &tags, counts).await,
            ["High daily usage"],
            "site prefix (counts={counts})"
        );
        assert_eq!(
            search(&store, "hvac", &tags, counts).await,
            ["Chiller flatline"],
            "subsystem, any case (counts={counts})"
        );
        // The title is still searched, and a term in BOTH a title and a tag matches both rows once.
        assert_eq!(
            search(&store, "ch", &tags, counts).await,
            ["Chiller flatline", "High daily usage"],
            "title OR tag, no duplicates (counts={counts})"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tag_that_is_not_configured_is_not_searched() {
    let store = seeded().await;
    // `meter` is on the row but not in the list: the search must not reach it.
    let none: Vec<String> = Vec::new();
    assert!(search(&store, "mdb", &keys(&["site"]), false)
        .await
        .is_empty());
    // With no keys configured the search is the title alone, exactly as before.
    assert!(search(&store, "chul", &none, false).await.is_empty());
    assert_eq!(
        search(&store, "door", &none, false).await,
        ["Door left open"]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_count_agrees_with_the_rows_under_a_tag_search() {
    let store = seeded().await;
    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter {
                search: Some("nsw".into()),
                search_tags: keys(&["state"]),
                ..Default::default()
            },
            cursor: None,
            offset: 0,
            // `limit: 0` is the aggregate-only path (`count`), a separate SQL statement.
            limit: 0,
            counts: true,
        },
        None,
        None,
    )
    .await
    .expect("count");
    assert_eq!(page.counts.expect("asked for counts").total, 1);
}

/// Every branch of the `OR` is answered from its index, never by a table scan: the whole point of
/// indexing the columns is that a search costs the matches, not the table.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tag_search_rides_the_indexes_not_a_table_scan() {
    let store = seeded().await;
    let tags = keys(&["site", "state"]);
    // The first search defines the indexes.
    search(&store, "ch", &tags, false).await;
    let mut r = store
        .query_ws(
            WS,
            "SELECT data FROM insight WHERE (data.title @0@ 'ch' OR data.tags.site @1@ 'ch' \
             OR data.tags.state @2@ 'ch') EXPLAIN",
            vec![],
        )
        .await
        .expect("explain");
    let rows: Vec<serde_json::Value> = r.take(0).expect("decode");
    let plan = serde_json::to_string(&rows).unwrap();
    println!("tag search plan: {plan}");
    assert!(
        !plan.contains("TableScan"),
        "must not scan the table: {plan}"
    );
    for index in [
        "insight_name_v2",
        "insight_tag_site_v2",
        "insight_tag_state_v2",
    ] {
        assert!(
            plan.contains(index),
            "{index} must answer its branch: {plan}"
        );
    }
}
