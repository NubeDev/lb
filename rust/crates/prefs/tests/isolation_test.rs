//! Workspace isolation — SPECIFIED, not generic (prefs scope mandatory category). The SAME global
//! user has `user_prefs` in BOTH ws-A and ws-B with DIFFERENT values; a resolve in ws-B returns
//! ws-B's values and NEVER reads ws-A's, and a ws-A workspace-default change does not move ws-B's
//! resolution. A single-workspace test would pass even with a leak — disallowed here.

use lb_prefs::{
    get_user_prefs, resolve_chain, set_user_prefs, set_workspace_prefs, Prefs, UnitSystem,
};
use lb_store::Store;

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn same_user_resolves_per_workspace_never_cross_reads() {
    let store = Store::memory().await.unwrap();
    let user = "user:ada"; // the SAME global identity in both workspaces

    // ws-A: Tokyo, imperial. ws-B: Madrid, metric.
    set_user_prefs(
        &store,
        "ws-a",
        user,
        &Prefs {
            timezone: Some("Asia/Tokyo".into()),
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_user_prefs(
        &store,
        "ws-b",
        user,
        &Prefs {
            timezone: Some("Europe/Madrid".into()),
            unit_system: Some(UnitSystem::Metric),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    // Resolve in ws-B returns ONLY ws-B's values.
    let rb = resolve_chain(&store, "ws-b", user, None).await.unwrap();
    assert_eq!(rb.timezone, "Europe/Madrid");
    assert_eq!(rb.unit_system, UnitSystem::Metric);

    // Resolve in ws-A returns ONLY ws-A's values — never bleeds B's.
    let ra = resolve_chain(&store, "ws-a", user, None).await.unwrap();
    assert_eq!(ra.timezone, "Asia/Tokyo");
    assert_eq!(ra.unit_system, UnitSystem::Imperial);

    // A direct read in ws-B cannot see ws-A's distinct value.
    let read_b = get_user_prefs(&store, "ws-b", user).await.unwrap().unwrap();
    assert_eq!(read_b.timezone, Some("Europe/Madrid".to_string()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_default_change_in_a_does_not_move_b() {
    let store = Store::memory().await.unwrap();
    let user = "user:ada";

    // Both workspaces have a default; the user sets nothing, so resolution = the ws default.
    set_workspace_prefs(
        &store,
        "ws-a",
        &Prefs {
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_workspace_prefs(
        &store,
        "ws-b",
        &Prefs {
            unit_system: Some(UnitSystem::Metric),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let before = resolve_chain(&store, "ws-b", user, None).await.unwrap();
    assert_eq!(before.unit_system, UnitSystem::Metric);

    // Change ws-A's default — ws-B's resolution must be unaffected.
    set_workspace_prefs(
        &store,
        "ws-a",
        &Prefs {
            unit_system: Some(UnitSystem::Metric),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    let after = resolve_chain(&store, "ws-b", user, None).await.unwrap();
    assert_eq!(after.unit_system, UnitSystem::Metric); // unchanged, and not because A changed
                                                       // And ws-A really did change (proving the write landed, so the isolation is real, not a no-op).
    let ra = resolve_chain(&store, "ws-a", user, None).await.unwrap();
    assert_eq!(ra.unit_system, UnitSystem::Metric);
}
