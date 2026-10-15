//! `Case::seq` — every case gets the workspace's next running number at open, two opens at once
//! never share one, and a workspace that had cases before numbering existed has them numbered in
//! opening order the first time a new case opens.

use std::collections::BTreeSet;

use lb_cases::{get, open, Grouping, OpenInput};
use lb_store::{delete, write, Store};

const WS: &str = "ops";

async fn case(store: &Store, n: u32) -> lb_cases::Case {
    open(
        store,
        WS,
        OpenInput {
            title: format!("case {n}"),
            grouping: Grouping::Single,
            primary_insight: format!("ins-{n}"),
            severity: "warning".into(),
            ..Default::default()
        },
        "user:test",
        1_000 + u64::from(n),
    )
    .await
    .expect("open")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cases_are_numbered_in_opening_order() {
    let store = Store::memory().await.expect("mem store");
    let seqs: Vec<_> = [
        case(&store, 1).await.seq,
        case(&store, 2).await.seq,
        case(&store, 3).await.seq,
    ]
    .into();
    assert_eq!(seqs, vec![Some(1), Some(2), Some(3)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_opens_never_share_a_number() {
    let store = Store::memory().await.expect("mem store");
    let opens = (1..=12).map(|n| {
        let store = store.clone();
        tokio::spawn(async move { case(&store, n).await.seq.expect("numbered") })
    });
    let mut got = BTreeSet::new();
    for h in opens {
        got.insert(h.await.expect("join"));
    }
    assert_eq!(got, (1..=12).collect::<BTreeSet<u64>>());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cases_from_before_numbering_are_backfilled_oldest_first() {
    let store = Store::memory().await.expect("mem store");
    // Two cases as an older node wrote them: no `seq`, and no counter record.
    let mut old = Vec::new();
    for n in [2, 1] {
        let mut c = case(&store, n).await;
        c.seq = None;
        write(
            &store,
            WS,
            "case",
            &c.id,
            &serde_json::to_value(&c).unwrap(),
        )
        .await
        .unwrap();
        old.push(c);
    }
    delete(&store, WS, "case_seq", "case").await.unwrap();

    let fresh = case(&store, 3).await;
    assert_eq!(
        fresh.seq,
        Some(3),
        "the new case follows the backfilled ones"
    );
    // Oldest first: case 1 opened at ts 1001, case 2 at 1002.
    let one = get(&store, WS, &old[1].id).await.unwrap().unwrap();
    let two = get(&store, WS, &old[0].id).await.unwrap().unwrap();
    assert_eq!((one.seq, two.seq), (Some(1), Some(2)));
    assert_eq!(
        one.last_activity_ts, old[1].last_activity_ts,
        "numbering is not activity"
    );
}
