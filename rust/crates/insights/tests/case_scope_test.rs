//! The case lens on `insight.list` — narrow and count detections by their case's stage — over a real
//! store, through the real `list` verb. The host resolves the case plane into a `CaseScope`; these
//! tests hand one in directly, because the crate's contract is what it does with it.
//!
//! What must hold: the pushed-down page (no counts) and the counted scan select the same rows; each
//! tally ignores its own axis only; and "no case" also covers an echo whose case is gone.

use std::collections::{BTreeMap, HashMap, HashSet};

use lb_insights::{
    list, raise, set_case_id, CaseScope, ListFilter, ListQuery, Origin, OriginKind, RaiseInput,
    Severity, NO_CASE,
};
use lb_store::Store;

const WS: &str = "ops";

async fn seed(store: &Store, key: &str, ts: u64, case: Option<&str>) {
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
            ts,
            producer: "user:test".into(),
        },
        50,
    )
    .await
    .expect("raise")
    .id;
    set_case_id(store, WS, &id, case).await.expect("echo");
}

/// a1, a2 → case A (actioned); b1 → case B (to_action); n1 → no case; g1 → case G, which the host
/// did not return (deleted after the echo was written).
async fn seeded() -> Store {
    let store = Store::memory().await.expect("mem store");
    seed(&store, "a1", 5, Some("case:A")).await;
    seed(&store, "a2", 4, Some("case:A")).await;
    seed(&store, "b1", 3, Some("case:B")).await;
    seed(&store, "n1", 2, None).await;
    seed(&store, "g1", 1, Some("case:G")).await;
    store
}

fn scope(allow: Option<&[&str]>, stages: Option<&[&str]>) -> CaseScope {
    let set = |xs: &[&str]| xs.iter().map(|x| x.to_string()).collect::<HashSet<_>>();
    CaseScope {
        stage_of: HashMap::from([
            ("case:A".to_string(), "actioned".to_string()),
            ("case:B".to_string(), "to_action".to_string()),
        ]),
        allow: allow.map(set),
        stages: stages.map(set),
        stage_order: vec!["to_action".into(), "actioned".into()],
    }
}

async fn page(store: &Store, case: CaseScope, counts: bool, limit: usize) -> lb_insights::ListPage {
    list(
        store,
        WS,
        ListQuery {
            filter: ListFilter {
                case: Some(case),
                ..Default::default()
            },
            cursor: None,
            offset: 0,
            limit,
            counts,
            sort: None,
        },
        None,
        None,
    )
    .await
    .expect("list")
}

fn titles(p: &lb_insights::ListPage) -> Vec<String> {
    p.items.iter().map(|i| i.title.clone()).collect()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_stage_filter_selects_the_same_rows_on_both_read_paths() {
    let store = seeded().await;
    for counts in [false, true] {
        let got = page(&store, scope(None, Some(&["actioned"])), counts, 50).await;
        assert_eq!(titles(&got), ["a1", "a2"], "actioned (counts={counts})");
        // "No case" is the detection without one AND the one whose case the host did not return.
        let got = page(&store, scope(None, Some(&[NO_CASE])), counts, 50).await;
        assert_eq!(titles(&got), ["n1", "g1"], "no case (counts={counts})");
        let got = page(
            &store,
            scope(None, Some(&["to_action", NO_CASE])),
            counts,
            50,
        )
        .await;
        assert_eq!(
            titles(&got),
            ["b1", "n1", "g1"],
            "a union (counts={counts})"
        );
        // The case filters' allowlist: a detection with no case never passes it.
        let got = page(&store, scope(Some(&["case:B"]), None), counts, 50).await;
        assert_eq!(titles(&got), ["b1"], "allowlist (counts={counts})");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_stage_tally_ignores_the_stage_filter_but_honours_the_case_filters() {
    let store = seeded().await;
    let got = page(&store, scope(None, Some(&["actioned"])), true, 50).await;
    let stages = got.case_counts.expect("counts asked, lens set");
    assert_eq!(stages.get("actioned"), Some(&2));
    assert_eq!(stages.get("to_action"), Some(&1));
    assert_eq!(stages.get(NO_CASE), Some(&2));
    // The status tally is over the picked stage: two actioned detections.
    assert_eq!(got.counts.expect("counts").total, 2);

    let got = page(&store, scope(Some(&["case:A"]), None), true, 50).await;
    let stages = got.case_counts.expect("counts");
    assert_eq!(stages.get("actioned"), Some(&2));
    assert_eq!(
        stages.get("to_action"),
        Some(&0),
        "the allowlist narrows the tally"
    );
    assert_eq!(stages.get(NO_CASE), Some(&0));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_counts_only_read_still_tallies_the_stages_and_pages_do_not() {
    let store = seeded().await;
    let got = page(&store, scope(None, None), true, 0).await;
    assert!(got.items.is_empty(), "limit 0 returns no rows");
    assert_eq!(got.case_counts.expect("counts").values().sum::<u64>(), 5);
    assert_eq!(got.counts.expect("counts").total, 5);

    // A later page asks for no counts and pays for no tally.
    let got = page(&store, scope(None, None), false, 2).await;
    assert!(got.case_counts.is_none() && got.counts.is_none());
    assert_eq!(titles(&got), ["a1", "a2"]);
}
