//! Per-severity counts — a roster's Critical / Warning / Info chips in ONE call. What must hold: the
//! tally ignores the severity filter (it IS the per-severity breakdown) but honours every other axis,
//! status included, and it is absent unless asked for.

use std::collections::BTreeMap;

use lb_insights::{
    list, raise, resolve, ListFilter, ListQuery, Origin, OriginKind, RaiseInput, Severity, Status,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, key: &str, severity: Severity) -> String {
    raise(
        store,
        WS,
        RaiseInput {
            dedup_key: key.into(),
            severity,
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
    .id
}

fn query(filter: ListFilter, limit: usize, severity_counts: bool) -> ListQuery {
    ListQuery {
        filter,
        cursor: None,
        offset: 0,
        limit,
        counts: true,
        sort: None,
        tag_counts: None,
        severity_counts,
    }
}

fn tally(c: u64, w: u64, i: u64) -> BTreeMap<String, u64> {
    [("critical", c), ("warning", w), ("info", i)]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn one_call_counts_every_severity_under_the_other_filters() {
    let store = Store::memory().await.expect("mem store");
    seed(&store, "c1", Severity::Critical).await;
    seed(&store, "c2", Severity::Critical).await;
    seed(&store, "w1", Severity::Warning).await;
    seed(&store, "i1", Severity::Info).await;
    let gone = seed(&store, "i2", Severity::Info).await;
    resolve(&store, WS, &gone, "user:test", None, 2)
        .await
        .expect("resolve");

    for limit in [0, 50] {
        // Every status: all five.
        let page = list(
            &store,
            WS,
            query(ListFilter::default(), limit, true),
            None,
            None,
        )
        .await
        .expect("list");
        assert_eq!(page.severity_counts, Some(tally(2, 1, 2)), "limit={limit}");

        // A severity filter narrows the ROWS but not the tally — the chips must still show all three.
        let crit_open = ListFilter {
            severity: Some(Severity::Critical),
            status: Some(Status::Open),
            ..Default::default()
        };
        let page = list(&store, WS, query(crit_open, limit, true), None, None)
            .await
            .expect("list");
        assert_eq!(
            page.severity_counts,
            Some(tally(2, 1, 1)),
            "status honoured (the resolved info is out), severity ignored (limit={limit})"
        );
        if limit > 0 {
            assert!(page.items.iter().all(|i| i.severity == Severity::Critical));
            assert_eq!(page.items.len(), 2);
        }
    }

    // An EXACT severity (a chip click) narrows the rows to that one severity; the tally still answers
    // all three, so the other chips keep their numbers.
    let exact_warning = ListFilter {
        severity: Some(Severity::Warning),
        severity_exact: true,
        ..Default::default()
    };
    for limit in [0, 50] {
        let page = list(
            &store,
            WS,
            query(exact_warning.clone(), limit, true),
            None,
            None,
        )
        .await
        .expect("list");
        assert_eq!(page.severity_counts, Some(tally(2, 1, 2)), "limit={limit}");
        if limit > 0 {
            let sevs: Vec<_> = page.items.iter().map(|i| i.severity).collect();
            assert_eq!(sevs, [Severity::Warning], "exact, not a floor");
        }
    }

    // Not asked ⇒ absent, so a reader can tell "not requested" from "none".
    let page = list(
        &store,
        WS,
        query(ListFilter::default(), 50, false),
        None,
        None,
    )
    .await
    .expect("list");
    assert!(page.severity_counts.is_none());
}
