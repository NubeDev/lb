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

/// The anti-rot invariant, mirrored as a runtime test (persona-grounding scope: "a SKILL.md on disk
/// not in the embedded corpus fails the build"). `build.rs` now PANICS the build if a `docs/skills/`
/// subdir lacks its `SKILL.md`; this test is the same gate expressed against the on-disk tree, so an
/// authoring mistake (an empty skill dir) fails LOUDLY here too — not silently as an absent skill a
/// persona then can't pin. If this test can't find the docs tree (e.g. a packaged crate), it no-ops.
#[test]
fn every_skills_dir_carries_a_skill_md() {
    let skills_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/skills");
    let Ok(read) = std::fs::read_dir(&skills_dir) else {
        return; // docs tree not present in this build context — the build.rs gate still holds.
    };
    for entry in read.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() {
            assert!(
                path.join("SKILL.md").exists(),
                "docs/skills/{:?} has no SKILL.md — the anti-rot gate (build.rs) rejects it; add the \
                 SKILL.md or remove the dir",
                path.file_name().unwrap_or_default()
            );
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_testing_runbooks_seed_as_core_skills() {
    // agent-personas #2 persona-grounding: the `docs/testing/**` e2e runbooks (which already carry
    // `e2e-*` skill frontmatter) now seed as `core.e2e-*` alongside the `docs/skills/*` corpus — so a
    // persona-grounded run learns "how do I verify this?" from the runbook, not by crawling the repo.
    // The embed pulls them from where they live (no copy that can drift). This asserts the second scan
    // root (docs/testing) in `build.rs` is wired and its ids land in the reserved namespace.
    let store = Store::memory().await.unwrap();
    let ids = seed_core_skills(&store, "0.1.0", 1).await.unwrap();
    for expected in ["core.e2e-backend", "core.e2e-frontend"] {
        assert!(
            ids.iter().any(|id| id == expected),
            "the runbook {expected} must seed as a core skill (docs/testing scan root)"
        );
    }
    // The README index under docs/testing/ has NO frontmatter and must NOT seed (it is a human index,
    // not a runbook skill) — no `core.README`/`core.testing` sneaks in.
    assert!(
        !ids.iter()
            .any(|id| id == "core.README" || id == "core.testing"),
        "the frontmatter-less docs/testing/README.md is skipped, not seeded"
    );
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
