//! Core tags behavior (tags scope): add → of → find (exact / key-only / faceted intersection),
//! remove, same-source upsert vs different-source coexistence, and the per-workspace tag-node cap.

use lb_tags::{add, check_cap, find, of, remove, Facet, Provenance, Source, Tag};
use lb_store::Store;

fn prov(at: u64, by: &str, source: Source) -> Provenance {
    Provenance::new(at, by, source)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn add_then_of_returns_the_tag() {
    let store = Store::memory().await.unwrap();
    add(
        &store,
        "acme",
        "series:cpu",
        &Tag::new("unit", "celsius".into()),
        &prov(1, "user:ada", Source::Producer),
        0,
    )
    .await
    .unwrap();

    let tags = of(&store, "acme", "series:cpu").await.unwrap();
    assert_eq!(tags.len(), 1);
    assert_eq!(tags[0].key, "unit");
    assert_eq!(tags[0].value, serde_json::json!("celsius"));
    assert_eq!(tags[0].source, Source::Producer);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn same_source_retag_upserts_different_source_coexists() {
    let store = Store::memory().await.unwrap();
    let tag = Tag::new("kind", "telemetry".into());
    // Human asserts.
    add(&store, "acme", "series:cpu", &tag, &prov(1, "user:ada", Source::Human), 0)
        .await
        .unwrap();
    // Same source re-tags with higher confidence — upserts in place (still ONE edge for human).
    let mut p = prov(2, "user:ada", Source::Human);
    p.confidence = 0.5;
    add(&store, "acme", "series:cpu", &tag, &p, 0).await.unwrap();
    // A DIFFERENT source (AI inferred) coexists as a second edge.
    add(&store, "acme", "series:cpu", &tag, &prov(3, "agent:x", Source::Inferred), 0)
        .await
        .unwrap();

    let tags = of(&store, "acme", "series:cpu").await.unwrap();
    assert_eq!(tags.len(), 2, "human (upserted) + inferred = two edges");
    let human = tags.iter().find(|t| t.source == Source::Human).unwrap();
    assert_eq!(human.confidence, 0.5, "same-source re-tag upserts confidence");
    assert!(tags.iter().any(|t| t.source == Source::Inferred));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn find_exact_key_only_and_faceted() {
    let store = Store::memory().await.unwrap();
    // cpu is eu-west telemetry; mem is eu-west event; disk is us-east telemetry.
    for (entity, region, kind) in [
        ("series:cpu", "eu-west", "telemetry"),
        ("series:mem", "eu-west", "event"),
        ("series:disk", "us-east", "telemetry"),
    ] {
        add(&store, "acme", entity, &Tag::new("region", region.into()), &prov(1, "p", Source::Producer), 0)
            .await
            .unwrap();
        add(&store, "acme", entity, &Tag::new("kind", kind.into()), &prov(1, "p", Source::Producer), 0)
            .await
            .unwrap();
    }

    // Exact: region=eu-west → cpu, mem.
    let eu = find(&store, "acme", &[Facet::exact("region", "eu-west".into())]).await.unwrap();
    assert_eq!(eu.len(), 2);
    assert!(eu.contains(&"series:cpu".to_string()) && eu.contains(&"series:mem".to_string()));

    // Key-only: has any region → all three.
    let any_region = find(&store, "acme", &[Facet::key_only("region")]).await.unwrap();
    assert_eq!(any_region.len(), 3);

    // Faceted intersection: eu-west AND telemetry → only cpu.
    let facet = find(
        &store,
        "acme",
        &[Facet::exact("region", "eu-west".into()), Facet::exact("kind", "telemetry".into())],
    )
    .await
    .unwrap();
    assert_eq!(facet, vec!["series:cpu".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remove_drops_the_edge_not_other_entities() {
    let store = Store::memory().await.unwrap();
    let tag = Tag::new("region", "eu".into());
    add(&store, "acme", "series:a", &tag, &prov(1, "p", Source::Producer), 0).await.unwrap();
    add(&store, "acme", "series:b", &tag, &prov(1, "p", Source::Producer), 0).await.unwrap();

    remove(&store, "acme", "series:a", "region", Some(&serde_json::json!("eu"))).await.unwrap();

    assert!(of(&store, "acme", "series:a").await.unwrap().is_empty());
    // series:b still points at the shared node.
    let b = find(&store, "acme", &[Facet::exact("region", "eu".into())]).await.unwrap();
    assert_eq!(b, vec!["series:b".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn tag_node_cap_denies_new_nodes_over_the_bound() {
    let store = Store::memory().await.unwrap();
    // Cap of 2 distinct tag nodes.
    add(&store, "acme", "ent:e1", &Tag::new("k", "a".into()), &prov(1, "p", Source::System), 2).await.unwrap();
    add(&store, "acme", "ent:e1", &Tag::new("k", "b".into()), &prov(1, "p", Source::System), 2).await.unwrap();
    // A THIRD distinct node is denied.
    let err = add(&store, "acme", "ent:e1", &Tag::new("k", "c".into()), &prov(1, "p", Source::System), 2).await;
    assert!(matches!(err, Err(lb_tags::AddError::CapExceeded(_))));

    // But re-using an EXISTING node (applying to a new entity) is allowed past the cap.
    add(&store, "acme", "ent:e2", &Tag::new("k", "a".into()), &prov(1, "p", Source::System), 2)
        .await
        .expect("re-using an existing tag node never counts toward the cap");

    // And the standalone cap check agrees.
    assert!(check_cap(&store, "acme", &Tag::new("k", "z".into()), 2).await.unwrap().is_err());
}
