//! Index-correctness for the SPIKE-GATED ADD-ONS the store spike marked AVAILABLE on SurrealKV
//! (tags scope): value full-text (`SEARCH`/BM25), vector (`HNSW`, with dimension pinned + mismatch
//! rejected), and the materialized per-dimension `tag_counts` view. If a future spike marks one
//! unavailable, that test is the gate that catches it.

use lb_store::Store;
use lb_tags::{
    add, count_by_key, define_counts_view, define_text_index, define_vector_index, find_similar,
    find_text, put_vector, Provenance, Source, Tag,
};

fn prov(at: u64) -> Provenance {
    Provenance::new(at, "p", Source::Producer)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn full_text_search_matches_tokenized_value() {
    let store = Store::memory().await.unwrap();
    define_text_index(&store, "acme").await.unwrap();
    add(
        &store,
        "acme",
        "series:cpu",
        &Tag::new("unit", "degrees celsius".into()),
        &prov(1),
        0,
    )
    .await
    .unwrap();
    add(
        &store,
        "acme",
        "series:net",
        &Tag::new("unit", "megabits".into()),
        &prov(1),
        0,
    )
    .await
    .unwrap();

    let hits = find_text(&store, "acme", "celsius").await.unwrap();
    assert_eq!(hits.len(), 1, "BM25 matches the tokenized value");
    assert_eq!(hits[0].0, "unit");
    assert_eq!(hits[0].1, serde_json::json!("degrees celsius"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn vector_search_returns_nearest_and_rejects_dim_mismatch() {
    let store = Store::memory().await.unwrap();
    let dim = 3;
    define_vector_index(&store, "acme", dim).await.unwrap();

    // Store three embeddings of the pinned dimension.
    put_vector(&store, "acme", "doc", "a", &[1.0, 0.0, 0.0], dim)
        .await
        .unwrap()
        .unwrap();
    put_vector(&store, "acme", "doc", "b", &[0.0, 1.0, 0.0], dim)
        .await
        .unwrap()
        .unwrap();
    put_vector(&store, "acme", "doc", "c", &[0.9, 0.1, 0.0], dim)
        .await
        .unwrap()
        .unwrap();

    // A mismatched-dimension write is REJECTED, never stored (index-corruption guard).
    let bad = put_vector(&store, "acme", "doc", "d", &[1.0, 0.0], dim)
        .await
        .unwrap();
    assert!(bad.is_err(), "a wrong-dim embedding must be rejected");

    // Nearest to [1,0,0] is "a" then "c" (find_similar returns the caller's logical ids).
    let near = find_similar(&store, "acme", &[1.0, 0.0, 0.0], 2)
        .await
        .unwrap();
    assert_eq!(
        near.first().map(String::as_str),
        Some("a"),
        "exact match is nearest"
    );
    assert!(
        near.contains(&"c".to_string()),
        "the close vector is in the top-2"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn materialized_counts_are_per_dimension() {
    let store = Store::memory().await.unwrap();
    define_counts_view(&store, "acme").await.unwrap();
    // Two region edges, one kind edge.
    add(
        &store,
        "acme",
        "series:a",
        &Tag::new("region", "eu".into()),
        &prov(1),
        0,
    )
    .await
    .unwrap();
    add(
        &store,
        "acme",
        "series:b",
        &Tag::new("region", "us".into()),
        &prov(1),
        0,
    )
    .await
    .unwrap();
    add(
        &store,
        "acme",
        "series:a",
        &Tag::new("kind", "telemetry".into()),
        &prov(1),
        0,
    )
    .await
    .unwrap();

    let counts = count_by_key(&store, "acme").await.unwrap();
    let region = counts.iter().find(|c| c.key == "region").unwrap();
    let kind = counts.iter().find(|c| c.key == "kind").unwrap();
    assert_eq!(region.n, 2, "two region edges");
    assert_eq!(kind.n, 1, "one kind edge");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn provenance_is_queryable_by_source() {
    use lb_tags::of;
    let store = Store::memory().await.unwrap();
    let tag = Tag::new("kind", "telemetry".into());
    add(
        &store,
        "acme",
        "series:x",
        &tag,
        &Provenance::new(1, "user:ada", Source::Human),
        0,
    )
    .await
    .unwrap();
    let mut inferred = Provenance::new(2, "agent:y", Source::Inferred);
    inferred.confidence = 0.92;
    add(&store, "acme", "series:x", &tag, &inferred, 0)
        .await
        .unwrap();

    let applied = of(&store, "acme", "series:x").await.unwrap();
    let inf = applied
        .iter()
        .find(|a| a.source == Source::Inferred)
        .unwrap();
    assert_eq!(inf.confidence, 0.92);
    assert_eq!(inf.by, "agent:y");
}
