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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_case_filter_finds_a_case_by_its_number() {
    let store = Store::memory().await.expect("mem store");
    let _one = case(&store, 1).await;
    let two = case(&store, 2).await;
    let filter = lb_cases::ListFilter {
        seq: Some(2),
        ..Default::default()
    };
    let hits: Vec<String> = lb_cases::stages(&store, WS, &filter, 5_000)
        .await
        .expect("stages")
        .into_iter()
        .filter(|c| c.matches)
        .map(|c| c.id)
        .collect();
    assert_eq!(hits, vec![two.id]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn part_of_a_number_finds_every_case_containing_it() {
    let store = Store::memory().await.expect("mem store");
    let mut by_seq = std::collections::BTreeMap::new();
    for n in 1..=12 {
        let c = case(&store, n).await;
        by_seq.insert(c.id.clone(), c.seq.unwrap());
    }
    let hits = |digits: &str| {
        let filter = lb_cases::ListFilter {
            seq_match: Some(lb_cases::SeqMatch {
                digits: digits.into(),
                width: 6,
            }),
            ..Default::default()
        };
        let store = store.clone();
        let by_seq = by_seq.clone();
        async move {
            let mut got: Vec<u64> = lb_cases::stages(&store, WS, &filter, 5_000)
                .await
                .expect("stages")
                .into_iter()
                .filter(|c| c.matches)
                .map(|c| by_seq[&c.id])
                .collect();
            got.sort();
            got
        }
    };
    // Written 000001, 000010, 000011, 000012: each contains "001".
    assert_eq!(hits("001").await, vec![1, 10, 11, 12]);
    assert_eq!(hits("12").await, vec![12]);
    assert_eq!(hits("x1").await, Vec::<u64>::new(), "only digits match");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_tick_numbers_existing_cases_without_waiting_for_a_new_one() {
    let store = Store::memory().await.expect("mem store");
    let mut old = Vec::new();
    for n in [1, 2] {
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
        old.push(c.id);
    }
    delete(&store, WS, "case_seq", "case").await.unwrap();

    assert_eq!(
        lb_cases::number_existing_cases(&store, WS, 9_000)
            .await
            .unwrap(),
        2
    );
    let seqs: Vec<_> = seqs_of(&store, &old).await;
    assert_eq!(seqs, vec![Some(1), Some(2)]);
    // Second pass: the counter exists, nothing to do.
    assert_eq!(
        lb_cases::number_existing_cases(&store, WS, 9_001)
            .await
            .unwrap(),
        0
    );
    // And the next open follows on.
    assert_eq!(case(&store, 3).await.seq, Some(3));
}

async fn seqs_of(store: &Store, ids: &[String]) -> Vec<Option<u64>> {
    let mut out = Vec::new();
    for id in ids {
        out.push(get(store, WS, id).await.unwrap().unwrap().seq);
    }
    out
}
