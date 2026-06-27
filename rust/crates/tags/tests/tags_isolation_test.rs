//! Mandatory workspace-isolation for tags — the highest-risk wall case (tags scope, testing §2.2).
//! Because `tag:[key,value]` is the SAME constructable record ID in every namespace, isolation rests
//! entirely on `use_ns` scoping — one forgotten one leaks across tenants. So this test constructs the
//! IDENTICAL `tag:['region','eu']` in BOTH ws-A and ws-B, writes `tagged` edges in each, and asserts
//! a ws-B find/traversal returns ZERO ws-A edges (and vice versa). A test using two DIFFERENT values
//! would pass even with a leak — so it is explicitly disallowed; this uses the same value on purpose.

use lb_store::Store;
use lb_tags::{add, find, of, Facet, Provenance, Source, Tag};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn identical_tag_does_not_leak_across_workspaces() {
    let store = Store::memory().await.unwrap();
    let region_eu = Tag::new("region", "eu".into());

    // The IDENTICAL tag id, in BOTH workspaces, each with its own entity edge.
    add(
        &store,
        "ws-a",
        "series:a",
        &region_eu,
        &Provenance::new(1, "user:a", Source::Producer),
        0,
    )
    .await
    .unwrap();
    add(
        &store,
        "ws-b",
        "series:b",
        &region_eu,
        &Provenance::new(1, "user:b", Source::Producer),
        0,
    )
    .await
    .unwrap();

    // ws-B find for region=eu must return ONLY ws-B's entity — never ws-A's, despite the same node id.
    let b_hits = find(&store, "ws-b", &[Facet::exact("region", "eu".into())])
        .await
        .unwrap();
    assert_eq!(
        b_hits,
        vec!["series:b".to_string()],
        "ws-B sees only its own edge"
    );

    // And vice versa.
    let a_hits = find(&store, "ws-a", &[Facet::exact("region", "eu".into())])
        .await
        .unwrap();
    assert_eq!(
        a_hits,
        vec!["series:a".to_string()],
        "ws-A sees only its own edge"
    );

    // Traversal (`of`) is likewise namespace-bound: ws-B's series:a (A's entity) has no tags in B.
    assert!(of(&store, "ws-b", "series:a").await.unwrap().is_empty());
    assert_eq!(of(&store, "ws-a", "series:a").await.unwrap().len(), 1);
}
