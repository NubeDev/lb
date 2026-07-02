//! Core-skill seeding into the reserved system namespace (core-skills scope): the embedded corpus
//! seeds `skill:core.<name>@<version>` records; re-seeding is idempotent (immutable versions);
//! a new version constant seeds new versions and leaves the old intact (rollback §6.4); and the
//! reserved namespace is separate from any workspace namespace (isolation).

use lb_assets::{
    get_core_skill, is_core, list_core_skill_versions, list_skills, seed_core_skills,
    CORE_SKILLS_NS,
};
use lb_store::Store;

#[test]
fn is_core_flags_the_reserved_prefix() {
    assert!(is_core("core.lb-cli"));
    assert!(is_core("core.query"));
    assert!(!is_core("acme-runbook"));
    assert!(!is_core("mycore")); // prefix must be `core.`, not just `core`
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seeding_is_idempotent_and_versioned() {
    let store = Store::memory().await.unwrap();

    // First seed at v0.1.0.
    let ids = seed_core_skills(&store, "0.1.0", 1).await.unwrap();
    assert!(!ids.is_empty(), "the embedded corpus is non-empty");
    assert!(
        ids.iter().all(|id| id.starts_with("core.")),
        "every seeded id is in the core namespace"
    );
    // The default-grant ids the scope names are present.
    for expected in ["core.lb-cli", "core.query", "core.store-read"] {
        assert!(ids.iter().any(|id| id == expected), "missing {expected}");
    }
    let first = ids.clone();

    // Re-seed the SAME version → a no-op: exactly one record per id (immutable versions).
    let again = seed_core_skills(&store, "0.1.0", 2).await.unwrap();
    assert_eq!(first, again, "re-seed returns the same id set");
    let v = list_core_skill_versions(&store, "core.lb-cli")
        .await
        .unwrap();
    assert_eq!(v.len(), 1, "one record for @0.1.0 after a double seed");
    assert_eq!(
        v[0].ts, 1,
        "the second seed did NOT overwrite the first (ts)"
    );

    // Upgrade: a new version constant seeds new versions; the old ones remain (rollback).
    seed_core_skills(&store, "0.2.0", 3).await.unwrap();
    let v = list_core_skill_versions(&store, "core.lb-cli")
        .await
        .unwrap();
    assert_eq!(v.len(), 2, "0.1.0 and 0.2.0 coexist");
    assert_eq!(v.last().unwrap().version, "0.2.0", "latest is the newest");
    // The old version is still individually resolvable (rollback / audit).
    assert!(get_core_skill(&store, "core.lb-cli", "0.1.0")
        .await
        .unwrap()
        .is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn core_records_live_outside_any_workspace_namespace() {
    // A seeded core skill lives in the reserved namespace, NOT in a workspace namespace — a workspace
    // `list_skills` for the same id sees nothing (the record is not in its namespace).
    let store = Store::memory().await.unwrap();
    seed_core_skills(&store, "0.1.0", 1).await.unwrap();

    // Present in the reserved namespace.
    assert!(!list_core_skill_versions(&store, "core.lb-cli")
        .await
        .unwrap()
        .is_empty());
    // Absent from an arbitrary workspace namespace (it was never written there).
    assert!(list_skills(&store, "acme", "core.lb-cli")
        .await
        .unwrap()
        .is_empty());
    // Sanity: the reserved namespace is the documented constant.
    assert_eq!(CORE_SKILLS_NS, "_lb_skills");
}
