//! Skill versioning: a published version is immutable; a new version coexists; rollback loads
//! a prior version (skills scope).

use lb_assets::{get_skill, list_skills, put_skill, Skill};
use lb_store::Store;

const WS: &str = "ws-skill-version";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn published_version_is_immutable() {
    let store = Store::memory().await.unwrap();
    put_skill(
        &store,
        WS,
        &Skill::new("s", "1.0.0", "a", "d", "v1 body", 1),
    )
    .await
    .unwrap();

    // Re-publishing the SAME version is rejected — versions are immutable.
    let err = put_skill(
        &store,
        WS,
        &Skill::new("s", "1.0.0", "a", "d", "tampered", 2),
    )
    .await
    .unwrap_err();
    assert!(err.to_string().contains("immutable"));

    // The original body is intact.
    let got = get_skill(&store, WS, "s", "1.0.0").await.unwrap().unwrap();
    assert_eq!(got.body, "v1 body");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn new_version_coexists_and_rollback_loads_prior() {
    let store = Store::memory().await.unwrap();
    put_skill(&store, WS, &Skill::new("s", "1.0.0", "a", "d", "v1", 1))
        .await
        .unwrap();
    put_skill(&store, WS, &Skill::new("s", "1.1.0", "a", "d", "v2", 2))
        .await
        .unwrap();

    let versions = list_skills(&store, WS, "s").await.unwrap();
    assert_eq!(versions.len(), 2);
    // Latest is last (ts order).
    assert_eq!(versions.last().unwrap().version, "1.1.0");
    // Rollback: a prior version's record never went away.
    assert_eq!(
        get_skill(&store, WS, "s", "1.0.0")
            .await
            .unwrap()
            .unwrap()
            .body,
        "v1"
    );
}
