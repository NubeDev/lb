//! Does the read-only session ALSO keep the secret plane out of reach? (Expected: NO.)
//!
//! `Store::query_ws_readonly` stops writes and stops cross-workspace reads, both enforced by the
//! engine. The secret-table wall is a THIRD protection and this records, rather than assumes,
//! whether the same mechanism covers it.
//!
//! `ctx::check_perms` bypasses table PERMISSIONS for `Action::View` when the actor holds the viewer
//! role AND its level covers the database — which is exactly our reader. So a viewer is expected to
//! read every table in its own workspace, secret tables included. If that is what happens, the
//! secret wall needs its own answer and cannot be folded into the session.

use lb_store::Store;
use serde_json::Value;

#[tokio::test]
async fn records_whether_a_viewer_can_read_the_secret_plane() {
    let s = Store::memory().await.expect("open");
    s.query_ws("ws-a", "CREATE secret:a SET v = 'top-secret';", vec![])
        .await
        .expect("seed")
        .check()
        .expect("ok");

    let mut r = s
        .query_ws_readonly("ws-a", "SELECT VALUE v FROM secret;", vec![])
        .await
        .expect("query runs");
    let rows: Vec<Value> = r.take(0).unwrap_or_default();
    println!("VIEWER-READS-SECRET: {rows:?}");
    assert_eq!(
        rows,
        vec![Value::from("top-secret")],
        "recorded: the viewer DOES read the secret plane, so the wall needs its own answer"
    );
}
