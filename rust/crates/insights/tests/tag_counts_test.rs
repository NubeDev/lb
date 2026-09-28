//! Per-value tag counts and the "tag missing" filter — what a roster's value cards and its grouped
//! sections read — over a real store, through the real `list` verb.
//!
//! What must hold: a value's count equals the rows a filter on that value returns, the rows without
//! the key are counted under `""` and are exactly what `tag_missing` returns, and a counts-only read
//! (`limit: 0`) still answers.

use std::collections::BTreeMap;

use lb_insights::{
    list, raise, set_tags_echo, ListFilter, ListQuery, Origin, OriginKind, RaiseInput, Severity,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, key: &str, site: Option<&str>) {
    let id = raise(
        store,
        WS,
        RaiseInput {
            dedup_key: key.into(),
            severity: Severity::Warning,
            title: key.into(),
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
            ts: 1,
            producer: "user:test".into(),
        },
        50,
    )
    .await
    .expect("raise")
    .id;
    let echo: BTreeMap<String, String> = site
        .map(|s| [("site".to_string(), s.to_string())].into())
        .unwrap_or_default();
    set_tags_echo(store, WS, &id, &echo).await.expect("echo");
}

fn query(filter: ListFilter, limit: usize, counts: bool, tag_counts: Option<&str>) -> ListQuery {
    ListQuery {
        filter,
        cursor: None,
        offset: 0,
        limit,
        counts,
        sort: None,
        tag_counts: tag_counts.map(str::to_string),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_value_is_counted_and_the_missing_ones_are_what_tag_missing_returns() {
    let store = Store::memory().await.expect("mem store");
    seed(&store, "a", Some("north")).await;
    seed(&store, "b", Some("north")).await;
    seed(&store, "c", Some("south")).await;
    seed(&store, "d", None).await;

    for limit in [0, 50] {
        let page = list(
            &store,
            WS,
            query(ListFilter::default(), limit, true, Some("site")),
            None,
            None,
        )
        .await
        .expect("list");
        let t = page.tag_counts.expect("asked for, with counts");
        assert_eq!(t.get("north"), Some(&2), "limit={limit}");
        assert_eq!(t.get("south"), Some(&1), "limit={limit}");
        assert_eq!(
            t.get(""),
            Some(&1),
            "the row without the key (limit={limit})"
        );
    }

    let missing = ListFilter {
        tag_missing: Some("site".into()),
        ..Default::default()
    };
    for counts in [false, true] {
        let page = list(
            &store,
            WS,
            query(missing.clone(), 50, counts, None),
            None,
            None,
        )
        .await
        .expect("list");
        let titles: Vec<_> = page.items.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["d"], "counts={counts}");
    }

    // Not asked for counts ⇒ no tally, even with a key.
    let page = list(
        &store,
        WS,
        query(ListFilter::default(), 50, false, Some("site")),
        None,
        None,
    )
    .await
    .expect("list");
    assert!(page.tag_counts.is_none());

    // A key that could break the SQL is refused, not spliced.
    let bad = list(
        &store,
        WS,
        query(ListFilter::default(), 50, true, Some("Site;x")),
        None,
        None,
    )
    .await;
    assert!(bad.is_err());
}
