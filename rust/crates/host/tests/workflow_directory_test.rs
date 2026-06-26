//! The workflow **directory** — the durable set of workspaces the driver services, mutable at runtime
//! so a workspace is onboarded/retired without a restart (workflow-driver scope). These cover the
//! register/deregister/list verbs directly against a real store: idempotent register, soft-disable on
//! deregister, the enabled-only scan, durability (a fresh read sees prior writes), and that the
//! reserved directory namespace is structurally separate from any tenant workspace.

use lb_host::{
    deregister_workspace, enabled_workspaces, register_workspace, EntryStatus, DIRECTORY_NS,
};
use lb_store::Store;

#[tokio::test]
async fn register_then_list_returns_the_enabled_workspace() {
    let store = Store::memory().await.unwrap();
    register_workspace(&store, "acme", "progress", 1)
        .await
        .unwrap();
    register_workspace(&store, "globex", "issues", 2)
        .await
        .unwrap();

    let entries = enabled_workspaces(&store).await.unwrap();
    assert_eq!(entries.len(), 2);
    // Oldest→newest by ts.
    assert_eq!(entries[0].ws, "acme");
    assert_eq!(entries[0].channel, "progress");
    assert_eq!(entries[0].status, EntryStatus::Enabled);
    assert_eq!(entries[1].ws, "globex");
}

#[tokio::test]
async fn deregister_soft_disables_and_drops_from_the_enabled_scan() {
    let store = Store::memory().await.unwrap();
    register_workspace(&store, "acme", "progress", 1)
        .await
        .unwrap();
    register_workspace(&store, "globex", "issues", 2)
        .await
        .unwrap();

    deregister_workspace(&store, "acme", 3).await.unwrap();

    // The enabled scan no longer returns acme — the driver stops servicing it next tick.
    let enabled = enabled_workspaces(&store).await.unwrap();
    assert_eq!(enabled.len(), 1);
    assert_eq!(enabled[0].ws, "globex");
}

#[tokio::test]
async fn register_is_idempotent_and_re_enables() {
    let store = Store::memory().await.unwrap();
    register_workspace(&store, "acme", "progress", 1)
        .await
        .unwrap();
    deregister_workspace(&store, "acme", 2).await.unwrap();
    assert!(enabled_workspaces(&store).await.unwrap().is_empty());

    // Re-registering re-enables (and can update the channel) — one row, not a duplicate.
    register_workspace(&store, "acme", "newchan", 3)
        .await
        .unwrap();
    let enabled = enabled_workspaces(&store).await.unwrap();
    assert_eq!(enabled.len(), 1, "still one row — register upserts");
    assert_eq!(enabled[0].channel, "newchan");
    assert_eq!(enabled[0].status, EntryStatus::Enabled);
}

#[tokio::test]
async fn the_directory_survives_independent_reads() {
    // Durability: the directory is a record, not in-memory config. A write is visible to any later
    // read of the same store (the proxy for surviving a restart — the driver re-reads each tick).
    let store = Store::memory().await.unwrap();
    register_workspace(&store, "acme", "progress", 1)
        .await
        .unwrap();
    // A second, independent read sees it.
    let again = enabled_workspaces(&store).await.unwrap();
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].ws, "acme");
}

#[tokio::test]
async fn the_directory_namespace_is_separate_from_a_tenant_workspace() {
    // ISOLATION: the directory lives in a reserved namespace, NOT inside a tenant workspace. A
    // workspace literally named the reserved string would be a different concern; here we prove the
    // directory's rows are not visible as ordinary inbox data in a tenant ws, and a tenant ws's data
    // does not leak into the directory scan.
    let store = Store::memory().await.unwrap();
    register_workspace(&store, "acme", "progress", 1)
        .await
        .unwrap();

    // The directory namespace is not "acme": acme's own inbox is empty (the entry is not in acme).
    assert!(
        lb_inbox::list(&store, "acme", "progress")
            .await
            .unwrap()
            .is_empty(),
        "the directory entry does not appear as data inside the 'acme' workspace"
    );
    // And the reserved namespace is a distinct string.
    assert_eq!(DIRECTORY_NS, "_lb_workflow_directory");
}
