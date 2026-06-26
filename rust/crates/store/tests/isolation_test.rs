//! MANDATORY workspace-isolation test at the STORE surface (testing §2.2): with a real
//! embedded SurrealDB (not a mock — testing §3), a record written in workspace A's namespace
//! is invisible to a read in workspace B, even at the same `table:id`. Isolation is structural
//! (README §7: workspace = namespace).

use lb_store::{read, write, Store};
use serde_json::json;

#[tokio::test]
async fn record_written_in_one_workspace_is_invisible_to_another() {
    let store = Store::memory().await.expect("open in-memory store");

    // Workspace A writes note:1.
    write(&store, "a", "note", "1", &json!({ "body": "secret of A" }))
        .await
        .expect("write to a");

    // Same table:id, but workspace B — must be absent (different namespace).
    let from_b = read(&store, "b", "note", "1").await.expect("read from b");
    assert!(
        from_b.is_none(),
        "workspace B must not see workspace A's record"
    );

    // Workspace A still reads its own.
    let from_a = read(&store, "a", "note", "1").await.expect("read from a");
    let value = from_a.expect("workspace A sees its own record");
    assert_eq!(value["body"], "secret of A");
}

#[tokio::test]
async fn same_id_in_two_workspaces_holds_independent_values() {
    let store = Store::memory().await.expect("open store");
    write(&store, "a", "note", "1", &json!({ "owner": "a" }))
        .await
        .unwrap();
    write(&store, "b", "note", "1", &json!({ "owner": "b" }))
        .await
        .unwrap();

    let a = read(&store, "a", "note", "1").await.unwrap().unwrap();
    let b = read(&store, "b", "note", "1").await.unwrap().unwrap();
    assert_eq!(a["owner"], "a");
    assert_eq!(b["owner"], "b");
}
