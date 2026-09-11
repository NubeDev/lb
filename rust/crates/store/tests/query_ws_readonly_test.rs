//! `Store::query_ws_readonly` — the engine-enforced read-only path behind `store.query`.
//!
//! These assert the two properties the verb rests on, at the level callers actually use, against a
//! real embedded store (no mocks): a write cannot land, and one workspace cannot read another.
//! `viewer_session_probe` / `viewer_namespace_probe` establish the same facts about raw SurrealDB;
//! these establish that `Store` wires them up correctly and keeps them across repeated use.

use lb_store::Store;
use serde_json::Value;

async fn seeded() -> Store {
    let s = Store::memory().await.expect("open");
    for (ws, v) in [("ws-a", "secret-a"), ("ws-b", "secret-b")] {
        s.query_ws(ws, &format!("CREATE thing:x SET v = '{v}';"), vec![])
            .await
            .expect("seed")
            .check()
            .expect("seed ok");
    }
    s
}

async fn values(s: &Store, ws: &str) -> Vec<Value> {
    let mut r = s
        .query_ws(ws, "SELECT VALUE v FROM thing;", vec![])
        .await
        .expect("read");
    r.take(0).expect("rows")
}

#[tokio::test]
async fn a_read_only_query_returns_rows() {
    let s = seeded().await;
    let mut r = s
        .query_ws_readonly("ws-a", "SELECT VALUE v FROM thing;", vec![])
        .await
        .expect("query runs");
    let rows: Vec<Value> = r.take(0).expect("rows");
    assert_eq!(rows, vec![Value::from("secret-a")]);
}

#[tokio::test]
async fn a_write_sent_through_the_read_only_path_does_not_land() {
    let s = seeded().await;
    for sql in [
        "CREATE thing:y SET v = 'nope';",
        "UPDATE thing:x SET v = 'clobbered';",
        "DELETE thing;",
        "REMOVE TABLE thing;",
    ] {
        // Whether this errors or comes back empty is the engine's business; that the data is
        // unchanged is ours.
        let _ = s.query_ws_readonly("ws-a", sql, vec![]).await;
        assert_eq!(
            values(&s, "ws-a").await,
            vec![Value::from("secret-a")],
            "data must be unchanged after: {sql}"
        );
    }
}

#[tokio::test]
async fn the_read_only_path_cannot_reach_another_workspace() {
    let s = seeded().await;
    // The caller writes their OWN `USE` to try to escape the prepended one. The old parse gate
    // refused a second statement; the engine now refuses the reach.
    let mut r = s
        .query_ws_readonly(
            "ws-a",
            "USE NS `ws-b` DB main;\nSELECT VALUE v FROM thing;",
            vec![],
        )
        .await
        .expect("query runs");
    let rows: Vec<Value> = r.take(1).unwrap_or_default();
    assert!(
        !rows.iter().any(|v| v == &Value::from("secret-b")),
        "a ws-a reader must not see ws-b rows, got {rows:?}"
    );
}

#[tokio::test]
async fn a_read_only_query_cannot_widen_its_own_permissions() {
    let s = seeded().await;
    for sql in [
        "DEFINE TABLE thing SCHEMALESS PERMISSIONS FULL;",
        "DEFINE USER escalate ON ROOT PASSWORD 'x' ROLES OWNER;",
    ] {
        let _ = s.query_ws_readonly("ws-a", sql, vec![]).await;
    }
    let _ = s
        .query_ws_readonly("ws-a", "CREATE thing:z SET v = 'esc';", vec![])
        .await;
    assert_eq!(values(&s, "ws-a").await, vec![Value::from("secret-a")]);
}

#[tokio::test]
async fn the_handle_is_reused_and_keeps_working() {
    let s = seeded().await;
    // The first call builds and caches the reader; later calls take the cached path. Both must
    // behave identically — a cached handle whose session drifted would be a silent wall failure.
    for _ in 0..5 {
        let mut r = s
            .query_ws_readonly("ws-a", "SELECT VALUE v FROM thing;", vec![])
            .await
            .expect("query runs");
        let rows: Vec<Value> = r.take(0).expect("rows");
        assert_eq!(rows, vec![Value::from("secret-a")]);
    }
    let _ = s.query_ws_readonly("ws-a", "DELETE thing;", vec![]).await;
    assert_eq!(values(&s, "ws-a").await, vec![Value::from("secret-a")]);
}

#[tokio::test]
async fn each_workspace_gets_its_own_reader() {
    let s = seeded().await;
    for (ws, want) in [("ws-a", "secret-a"), ("ws-b", "secret-b")] {
        let mut r = s
            .query_ws_readonly(ws, "SELECT VALUE v FROM thing;", vec![])
            .await
            .expect("query runs");
        let rows: Vec<Value> = r.take(0).expect("rows");
        assert_eq!(rows, vec![Value::from(want)], "for {ws}");
    }
}

#[tokio::test]
async fn bindings_still_work_on_the_read_only_path() {
    let s = seeded().await;
    let mut r = s
        .query_ws_readonly(
            "ws-a",
            "SELECT VALUE v FROM thing WHERE v = $want;",
            vec![("want".into(), Value::from("secret-a"))],
        )
        .await
        .expect("query runs");
    let rows: Vec<Value> = r.take(0).expect("rows");
    assert_eq!(rows, vec![Value::from("secret-a")]);
}

#[tokio::test]
async fn an_invalid_workspace_is_refused_before_the_engine() {
    let s = seeded().await;
    let out = s
        .query_ws_readonly("ws a; DROP", "SELECT * FROM thing;", vec![])
        .await;
    assert!(out.is_err(), "the workspace charset check must still apply");
}
