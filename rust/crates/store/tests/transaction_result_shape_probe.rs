//! Where does a transaction's `RETURN` value land in the response?
//!
//! `lb_jobs::retain` reads its delete count with `take(0)` on the strength of a comment that says
//! "a transaction whose body ends in `RETURN` collapses to a SINGLE result set at index 0 (the
//! RETURN value), not one-per-statement". If that stopped being true, `take(0)` silently returns the
//! WRONG statement's result — and because the caller ends with `unwrap_or(0)`, the wrong answer is
//! reported as "deleted 0 rows" rather than as an error. Retention that reports 0 while doing
//! nothing is exactly how a disc fills up quietly.
//!
//! This probe pins the real shape instead of trusting the comment.

use lb_store::Store;
use serde_json::Value;

#[tokio::test]
async fn where_a_transactions_return_value_lands() {
    let store = Store::memory().await.expect("open");
    for i in 0..5u32 {
        store
            .query_ws(
                "ws-a",
                &format!("CREATE j:k{i} SET data = {{ n: {i} }}"),
                vec![],
            )
            .await
            .expect("seed");
    }

    let mut resp = store
        .query_ws(
            "ws-a",
            "BEGIN TRANSACTION;\
             LET $keep = (SELECT VALUE id FROM j ORDER BY id DESC LIMIT 2);\
             LET $doomed = (SELECT VALUE <string>id FROM j WHERE id NOT IN $keep);\
             DELETE FROM j WHERE <string>id IN $doomed;\
             RETURN count($doomed);\
             COMMIT TRANSACTION;",
            vec![],
        )
        .await
        .expect("the retain transaction must run");

    for i in 0..6 {
        match resp.take::<Vec<Value>>(i) {
            Ok(v) => println!("index {i}: {v:?}"),
            Err(e) => println!("index {i}: <error> {e}"),
        }
    }

    // Whatever the indexing, the DELETE itself must have happened: 5 rows, keep 2, so 3 go.
    let mut check = store
        .query_ws("ws-a", "SELECT VALUE <string>id FROM j", vec![])
        .await
        .expect("count survivors");
    let left: Vec<Value> = check.take(0).expect("take");
    println!("survivors: {} -> {left:?}", left.len());
    assert_eq!(
        left.len(),
        2,
        "the transaction's DELETE must really have run"
    );
}
