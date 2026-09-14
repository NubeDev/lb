//! The two indexes must actually be USED — over a real store, checked against the engine's own plan.
//!
//! Asserting "an index exists" proves nothing: SurrealDB's planner is free to ignore one, and every
//! lb record is wrapped as `{ data: <host json>, rev: n }`, so these are indexes on NESTED paths
//! (`data.dedup_key`, `data.title`) rather than plain columns. That is exactly the case that could
//! silently fall back to a scan, so the plan is the assertion.

use std::collections::BTreeMap;

use lb_insights::{list, raise, ListFilter, ListQuery, Origin, OriginKind, RaiseInput, Severity};
use lb_store::Store;

const WS: &str = "ops";
const RING: usize = 50;

async fn seed(store: &Store, key: &str, title: &str) -> String {
    raise(
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
        RING,
    )
    .await
    .expect("raise")
    .id
}

/// The operators in the engine's plan for `sql`, flattened depth-first.
async fn plan(store: &Store, sql: &str) -> String {
    let mut r = store
        .query_ws(WS, &format!("{sql} EXPLAIN"), vec![])
        .await
        .expect("explain");
    let rows: Vec<serde_json::Value> = r.take(0).expect("decode");
    fn walk(v: &serde_json::Value, out: &mut Vec<String>) {
        // Two shapes in the wild: `operator` (SelectProject/TableScan/IndexScan) and `operation`
        // (Iterate Index/Collector, which is what a full-text plan reports).
        for key in ["operator", "operation"] {
            if let Some(op) = v.get(key).and_then(|o| o.as_str()) {
                out.push(op.to_string());
            }
        }
        if let Some(kids) = v.get("children").and_then(|c| c.as_array()) {
            for k in kids {
                walk(k, out);
            }
        }
    }
    let mut ops = Vec::new();
    for row in &rows {
        walk(row, &mut ops);
    }
    ops.join(" → ")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_dedup_lookup_stops_being_a_table_scan() {
    let store = Store::memory().await.expect("mem store");
    for i in 0..40 {
        seed(
            &store,
            &format!("common:met-003:pnt_{i}"),
            &format!("Meter flatline {i}"),
        )
        .await;
    }

    // This is the query `insight_id::dedup_lookup` runs on EVERY raise.
    let sql = "SELECT data FROM insight WHERE data.dedup_key = 'common:met-003:pnt_7'";
    let p = plan(&store, sql).await;
    println!("dedup plan: {p}");
    assert!(
        p.contains("IndexScan"),
        "the dedup lookup must use insight_dedup, got: {p}"
    );
    assert!(
        !p.contains("TableScan"),
        "it must NOT fall back to a scan, got: {p}"
    );

    // The plan is worthless if it changes the answer.
    let mut r = store.query_ws(WS, sql, vec![]).await.expect("read");
    let rows: Vec<serde_json::Value> = r.take(0).expect("decode");
    assert_eq!(
        rows.len(),
        1,
        "the indexed lookup must still find exactly one row"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn name_search_uses_the_index_and_searches_every_row() {
    let store = Store::memory().await.expect("mem store");
    seed(&store, "k1", "Solar meter flatline").await;
    seed(&store, "k2", "Water meter flatline").await;
    seed(&store, "k3", "AHU supply temp above setpoint").await;
    // Enough noise that a window-based search would miss the match at the far end.
    for i in 0..60 {
        seed(&store, &format!("noise{i}"), &format!("Routine check {i}")).await;
    }

    // NB: a full-text index reports as `Iterate Index`, not `IndexScan` — a different plan shape
    // from the equality case above, so the assertion has to name the right operator.
    let p = plan(
        &store,
        "SELECT data FROM insight WHERE data.title @@ 'flatline'",
    )
    .await;
    println!("search plan: {p}");
    assert!(
        p.contains("Iterate Index") || p.contains("IndexScan"),
        "name search must ride the BM25 index, got: {p}"
    );

    let page = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter {
                search: Some("flatline".into()),
                ..Default::default()
            },
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

    let titles: Vec<&str> = page.items.iter().map(|i| i.title.as_str()).collect();
    assert_eq!(
        titles.len(),
        2,
        "only the two flatline findings match, got {titles:?}"
    );
    assert!(titles.iter().all(|t| t.to_lowercase().contains("flatline")));

    // A term nobody has: an honest empty answer, not everything.
    let none = list(
        &store,
        WS,
        ListQuery {
            filter: ListFilter {
                search: Some("compressor".into()),
                ..Default::default()
            },
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
    assert!(none.items.is_empty(), "no match must mean no rows");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_prefix_finds_the_word_because_that_is_how_a_search_box_is_used() {
    let store = Store::memory().await.expect("mem store");
    // Verbatim title shapes from the ESR store — hyphens, digits, and all.
    seed(
        &store,
        "k1",
        "High Daily Usage - MSB1 - CH3 - Total Active Energy Import",
    )
    .await;
    seed(
        &store,
        "k2",
        "Flatline - MDB-1-4 - Total Active Energy Import",
    )
    .await;
    seed(&store, "k3", "AHU-2 supply temp above setpoint").await;

    for (term, expect, why) in [
        (
            "hi",
            1usize,
            "a two-letter prefix must find `High` — the reported bug",
        ),
        ("high", 1, "the whole word still matches"),
        ("flat", 1, "prefix of `Flatline`"),
        ("mdb", 1, "prefix INSIDE the hyphenated token `MDB-1-4`"),
        (
            "msb1",
            1,
            "letters+digits stay one token (why `class` is not a tokenizer here)",
        ),
        ("usage", 1, "a word in the middle of the title"),
        ("compressor", 0, "a term nobody has must return nothing"),
    ] {
        let page = list(
            &store,
            WS,
            ListQuery {
                filter: ListFilter {
                    search: Some(term.into()),
                    ..Default::default()
                },
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
        assert_eq!(page.items.len(), expect, "searching {term:?}: {why}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_letter_is_not_a_search_and_must_not_empty_the_roster() {
    let store = Store::memory().await.expect("mem store");
    seed(&store, "k1", "High Daily Usage - MSB1 - CH3").await;
    seed(&store, "k2", "Flatline - MDB-1-4").await;
    seed(&store, "k3", "AHU-2 supply temp above setpoint").await;

    // Reported from the browser: typing `h` emptied the list and said "no findings match". The
    // analyzer indexes prefixes from length 2, so one letter can never match — and a roster that
    // blanks on the first keystroke looks broken. Below the floor it is NO filter, not an empty one.
    for (term, expect, why) in [
        ("h", 3usize, "one letter is not a search — show everything"),
        (" h ", 3, "whitespace does not make it longer"),
        ("", 3, "an empty box is not a filter"),
        ("hi", 1, "two characters DO search"),
    ] {
        let page = list(
            &store,
            WS,
            ListQuery {
                filter: ListFilter {
                    search: Some(term.into()),
                    ..Default::default()
                },
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
        assert_eq!(page.items.len(), expect, "searching {term:?}: {why}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offset_pages_the_roster_without_repeating_or_skipping() {
    let store = Store::memory().await.expect("mem store");
    // Distinct timestamps so the order is total and the assertions are exact.
    for i in 0..25u64 {
        raise(
            &store,
            WS,
            RaiseInput {
                dedup_key: format!("k{i:02}"),
                severity: Severity::Warning,
                title: format!("Finding {i:02}"),
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
                ts: 1_788_000_000_000 + i,
                producer: "user:test".into(),
            },
            RING,
        )
        .await
        .expect("raise");
    }

    async fn page_at(store: &Store, offset: usize) -> lb_insights::ListPage {
        list(
            store,
            WS,
            ListQuery {
                filter: ListFilter::default(),
                cursor: None,
                limit: 10,
                offset,
                counts: false,
            },
            None,
            None,
        )
        .await
        .expect("list")
    }

    let p1 = page_at(&store, 0).await;
    let p2 = page_at(&store, 10).await;
    let p3 = page_at(&store, 20).await;

    assert_eq!(p1.items.len(), 10, "page 1 is a full page");
    assert_eq!(p2.items.len(), 10, "page 2 is a full page");
    assert_eq!(p3.items.len(), 5, "the last page is the remainder of 25");

    // The property a pager lives or dies on: no row appears twice, none is skipped.
    let mut seen: Vec<&str> = p1
        .items
        .iter()
        .chain(&p2.items)
        .chain(&p3.items)
        .map(|i| i.id.as_str())
        .collect();
    let total = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), total, "paging must not repeat a row");
    assert_eq!(total, 25, "paging must not skip a row");

    // Newest first, so page 1 starts at the highest ts and pages descend.
    assert!(
        p1.items[0].last_ts > p2.items[0].last_ts,
        "pages must descend"
    );
    assert!(
        p2.items[0].last_ts > p3.items[0].last_ts,
        "pages must descend"
    );

    // Past the end is empty, not a wrap-around.
    let beyond = page_at(&store, 999).await;
    assert!(
        beyond.items.is_empty(),
        "an offset past the end returns nothing"
    );
}
