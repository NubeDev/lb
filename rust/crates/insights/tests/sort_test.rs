//! The column sort over a real store, through the real `list` verb. The property that matters most is
//! that a counted page 1 (the scan path) and an uncounted page 2 (the pushed-down path) are slices of
//! ONE order, so paging a sorted roster never repeats or skips a row.

use std::collections::{BTreeMap, HashMap};

use lb_insights::{
    list, raise, set_case_id, set_tags_echo, CaseScope, ListFilter, ListQuery, Origin, OriginKind,
    PageCursor, RaiseInput, Severity, SortSpec,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, title: &str, sev: Severity, ts: u64, tags: &[(&str, &str)]) -> String {
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
            ts,
            producer: "user:test".into(),
        },
        50,
    )
    .await
    .expect("raise")
    .id;
    let echo = tags
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    set_tags_echo(store, WS, &id, &echo).await.expect("echo");
    id
}

/// Names (short_name, else title): "alpha", "Bravo", "charlie", "delta"; "echo" has no site.
async fn seeded() -> Store {
    let store = Store::memory().await.expect("mem store");
    seed(
        &store,
        "t-delta",
        Severity::Info,
        1,
        &[("short_name", "delta"), ("site", "S2")],
    )
    .await;
    seed(&store, "Bravo", Severity::Critical, 2, &[("site", "s1")]).await;
    seed(
        &store,
        "t-alpha",
        Severity::Warning,
        3,
        &[("short_name", "alpha"), ("site", "S3")],
    )
    .await;
    seed(&store, "charlie", Severity::Critical, 4, &[("site", "s4")]).await;
    seed(&store, "echo", Severity::Warning, 5, &[]).await;
    store
}

fn sort(by: &[&str], desc: bool) -> Option<SortSpec> {
    Some(SortSpec {
        by: by.iter().map(|s| s.to_string()).collect(),
        desc,
    })
}

async fn read(
    store: &Store,
    s: Option<SortSpec>,
    case: Option<CaseScope>,
    offset: usize,
    limit: usize,
    counts: bool,
) -> Vec<String> {
    list(
        store,
        WS,
        ListQuery {
            filter: ListFilter {
                case,
                ..Default::default()
            },
            cursor: None,
            offset,
            limit,
            counts,
            sort: s,
        },
        None,
        None,
    )
    .await
    .expect("list")
    .items
    .into_iter()
    .map(|i| i.title)
    .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_chained_text_sort_is_case_insensitive_and_puts_blanks_last_both_ways() {
    let store = seeded().await;
    let name = ["tag:short_name", "title"];
    for counts in [false, true] {
        assert_eq!(
            read(&store, sort(&name, false), None, 0, 50, counts).await,
            ["t-alpha", "Bravo", "charlie", "t-delta", "echo"],
            "name asc (counts={counts})"
        );
        // "echo" has no site: last ascending AND descending.
        assert_eq!(
            read(&store, sort(&["tag:site"], false), None, 0, 50, counts).await,
            ["Bravo", "t-delta", "t-alpha", "charlie", "echo"],
            "site asc (counts={counts})"
        );
        assert_eq!(
            read(&store, sort(&["tag:site"], true), None, 0, 50, counts).await,
            ["charlie", "t-alpha", "t-delta", "Bravo", "echo"],
            "site desc (counts={counts})"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_counted_page_one_and_an_uncounted_page_two_are_one_order() {
    let store = seeded().await;
    // Severity descending, ties newest first: the roster's shipped default.
    let s = || sort(&["severity"], true);
    let whole = read(&store, s(), None, 0, 50, false).await;
    assert_eq!(whole, ["charlie", "Bravo", "echo", "t-alpha", "t-delta"]);
    let mut paged = read(&store, s(), None, 0, 2, true).await;
    paged.extend(read(&store, s(), None, 2, 2, false).await);
    paged.extend(read(&store, s(), None, 4, 2, false).await);
    assert_eq!(
        paged, whole,
        "no row repeated or skipped across the two paths"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_case_stage_sort_follows_the_hosts_order_and_no_case_is_last() {
    let store = Store::memory().await.expect("mem store");
    let a = seed(&store, "a", Severity::Info, 1, &[]).await;
    let b = seed(&store, "b", Severity::Info, 2, &[]).await;
    seed(&store, "c", Severity::Info, 3, &[]).await;
    set_case_id(&store, WS, &a, Some("case:A")).await.unwrap();
    set_case_id(&store, WS, &b, Some("case:B")).await.unwrap();
    let scope = CaseScope {
        stage_of: HashMap::from([
            ("case:A".into(), "resolved".into()),
            ("case:B".into(), "to_action".into()),
        ]),
        stage_order: vec!["to_action".into(), "actioned".into(), "resolved".into()],
        ..Default::default()
    };
    for counts in [false, true] {
        let got = read(
            &store,
            sort(&["case_stage"], false),
            Some(scope.clone()),
            0,
            50,
            counts,
        )
        .await;
        assert_eq!(got, ["b", "a", "c"], "counts={counts}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bad_sort_or_a_sorted_cursor_is_refused() {
    let store = seeded().await;
    let q = |s: Option<SortSpec>, cursor: Option<PageCursor>| ListQuery {
        filter: ListFilter::default(),
        cursor,
        offset: 0,
        limit: 5,
        counts: false,
        sort: s,
    };
    for bad in [
        sort(&["tag:Site"], false),
        sort(&["severity", "title"], false),
    ] {
        assert!(list(&store, WS, q(bad, None), None, None).await.is_err());
    }
    let cursor = PageCursor {
        ts: 9,
        id: "x".into(),
    };
    assert!(
        list(
            &store,
            WS,
            q(sort(&["title"], false), Some(cursor)),
            None,
            None
        )
        .await
        .is_err(),
        "a keyset cursor walks the default order only"
    );
}
