//! Store-layer workspace isolation for assets (mandatory category, testing §2.2).
//!
//! These are the raw store verbs *below* the host's capability/membership gate. They must
//! already honor the hard wall structurally: a read/list in workspace B can never see workspace
//! A's docs, skills, relations, or install records — because the namespace is selected from
//! `ws` (README §7). The host gates add capability + membership *on top*; this proves the
//! foundation they stand on.

use lb_assets::{
    get_doc, get_skill, list_docs, list_related, list_skills, put_doc, put_skill, read_install,
    record_install, relate, related, Doc, Install, Skill,
};
use lb_store::Store;

const WS_A: &str = "ws-assets-iso-a";
const WS_B: &str = "ws-assets-iso-b";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn doc_written_in_a_is_invisible_in_b() {
    let store = Store::memory().await.unwrap();
    put_doc(&store, WS_A, &Doc::new("d1", "user:ada", "T", "secret", 1))
        .await
        .unwrap();

    // Same id, other workspace → nothing.
    assert!(get_doc(&store, WS_B, "d1").await.unwrap().is_none());
    assert!(list_docs(&store, WS_B, "user:ada")
        .await
        .unwrap()
        .is_empty());
    // And it IS there in A.
    assert!(get_doc(&store, WS_A, "d1").await.unwrap().is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn skill_published_in_a_is_invisible_in_b() {
    let store = Store::memory().await.unwrap();
    put_skill(
        &store,
        WS_A,
        &Skill::new("s1", "1.0.0", "user:ada", "d", "body", 1),
    )
    .await
    .unwrap();

    assert!(get_skill(&store, WS_B, "s1", "1.0.0")
        .await
        .unwrap()
        .is_none());
    assert!(list_skills(&store, WS_B, "s1").await.unwrap().is_empty());
    assert!(get_skill(&store, WS_A, "s1", "1.0.0")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn relation_in_a_is_invisible_in_b() {
    let store = Store::memory().await.unwrap();
    relate(&store, WS_A, "member", "team:eng", "user:ben")
        .await
        .unwrap();

    assert!(related(&store, WS_A, "member", "team:eng", "user:ben")
        .await
        .unwrap());
    assert!(!related(&store, WS_B, "member", "team:eng", "user:ben")
        .await
        .unwrap());
    assert!(list_related(&store, WS_B, "member", "team:eng")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn install_record_in_a_is_invisible_in_b() {
    let store = Store::memory().await.unwrap();
    record_install(
        &store,
        WS_A,
        &Install::new("hello", "0.1.0", vec!["store:note:read".into()], 1),
    )
    .await
    .unwrap();

    assert!(read_install(&store, WS_B, "hello").await.unwrap().is_none());
    let got = read_install(&store, WS_A, "hello").await.unwrap().unwrap();
    assert_eq!(got.granted, vec!["store:note:read".to_string()]);
}
