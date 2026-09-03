//! Does SurrealDB 3 REFUSE a mutation, or merely choose a write transaction for it?
//! `kvs/ds.rs` picks `TransactionType::Read` when `val.read_only()`. This asks whether that is
//! enforcement (a write is refused) or just an optimisation (a write gets a write txn and runs).
use lb_store::Store;

#[tokio::test]
async fn does_a_mutation_run_when_we_only_wanted_reads() {
    let store = Store::memory().await.expect("open");
    store
        .query_ws(
            "probe",
            "CREATE type::record('t', 'a') CONTENT { v: 1 };",
            vec![],
        )
        .await
        .expect("seed write");
    // A read: expected to work.
    let r = store.query_ws("probe", "SELECT * FROM t;", vec![]).await;
    println!("SELECT  -> {}", if r.is_ok() { "ok" } else { "ERR" });
    // A mutation issued through the very same path. If SurrealDB enforced read-only anywhere
    // reachable from here, this would fail.
    let d = store.query_ws("probe", "DELETE t;", vec![]).await;
    println!(
        "DELETE  -> {}",
        if d.is_ok() {
            "ok — NOT refused"
        } else {
            "ERR — refused"
        }
    );
    assert!(
        d.is_ok(),
        "if this ever fails, SurrealDB gained an enforcement path worth using"
    );
}
