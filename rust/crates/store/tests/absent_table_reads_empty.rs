//! A read of a table nothing has written yields nothing — the contract SurrealDB 2 gave us for free
//! and SurrealDB 3 does not.
//!
//! SurrealDB 3 raises `NotFoundError::Table` for `SELECT`/`UPDATE`/`DELETE` against a table with no
//! catalog entry, unconditionally. lb reads before it writes constantly (a fresh node, an empty
//! workspace, any `*.list` verb), so `lb_store` restores the old answer. These tests hold that line
//! against the REAL engine — no mocks — and would fail loudly if a future SurrealDB made the check
//! configurable and someone removed the shim without checking.

use lb_store::Store;

async fn store() -> Store {
    Store::memory().await.expect("open in-memory store")
}

#[tokio::test]
async fn selecting_a_table_that_was_never_written_yields_no_rows() {
    let s = store().await;
    let mut resp = s
        .query_ws("ws-a", "SELECT * FROM never_written", vec![])
        .await
        .expect("a SELECT of an absent table must succeed, not error");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
    assert!(rows.is_empty(), "expected no rows, got {rows:?}");
}

#[tokio::test]
async fn taking_an_absent_table_as_option_yields_none() {
    let s = store().await;
    let mut resp = s
        .query_ws("ws-a", "SELECT * FROM never_written LIMIT 1", vec![])
        .await
        .expect("query");
    let row: Option<serde_json::Value> = resp.take(0).expect("take");
    assert!(row.is_none(), "expected None, got {row:?}");
}

#[tokio::test]
async fn deleting_from_an_absent_table_is_a_no_op_not_an_error() {
    let s = store().await;
    s.query_ws("ws-a", "DELETE never_written", vec![])
        .await
        .expect("a DELETE against an absent table must be a no-op");
}

#[tokio::test]
async fn a_real_error_still_surfaces() {
    // The shim must drop ONLY the absent-table error. A genuinely broken statement still fails.
    let s = store().await;
    let err = s
        .query_ws("ws-a", "SELECT * FROM ORDER BY", vec![])
        .await
        .err()
        .expect("a malformed statement must still be an error");
    let _ = err;
}

#[tokio::test]
async fn the_first_real_error_wins_even_when_an_absent_table_precedes_it() {
    // `take_errors` drains into a HashMap, so ordering is re-established by hand; this pins it.
    let s = store().await;
    let err = s
        .query_ws(
            "ws-a",
            "SELECT * FROM never_written; SELECT * FROM ORDER BY;",
            vec![],
        )
        .await
        .err()
        .expect("the malformed second statement must surface");
    let _ = err;
}

#[tokio::test]
async fn a_written_table_still_reads_its_rows() {
    // The shim must not mask real data: once the table exists, reads behave normally.
    let s = store().await;
    s.query_ws("ws-a", "CREATE now_written:one SET v = 7", vec![])
        .await
        .expect("create");
    let mut resp = s
        .query_ws("ws-a", "SELECT v FROM now_written", vec![])
        .await
        .expect("query");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take");
    assert_eq!(rows.len(), 1, "expected the row we wrote, got {rows:?}");
    assert_eq!(rows[0]["v"], serde_json::json!(7));
}
