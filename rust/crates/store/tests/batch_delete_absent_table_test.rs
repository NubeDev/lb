//! A batched DELETE against a table that was never written must be a clean no-op.
//!
//! SurrealDB 2 answered a `DELETE` on an unwritten table with no rows. SurrealDB 3 raises
//! `NotFoundError::Table`. `query_ws` drops that error for a bare statement, but it cannot help
//! inside a transaction: the failed statement aborts the whole transaction and the COMMIT then
//! fails with `Cannot COMMIT: the transaction was aborted due to a prior error`, which names no
//! table and so cannot be recognised after the fact.
//!
//! The consequence was not theoretical. `roles.delete` cascades through `write_batch`, so a
//! workspace-B admin deleting a role that lives only in workspace A — which must succeed with
//! `affected: 0`, proving isolation by scope rather than by refusal — failed the whole request,
//! and the gateway turned that into a 403.

use lb_store::{read, write, write_batch, DeleteBatch, Store, UpsertBatch};
use serde_json::json;

#[tokio::test]
async fn a_batch_delete_on_a_never_written_table_is_a_no_op() {
    let s = Store::memory().await.expect("open");
    let deletes = vec![DeleteBatch {
        table: "role",
        id: "operator",
    }];
    write_batch(&s, "fresh", &[], &deletes)
        .await
        .expect("deleting nothing from an absent table is a no-op, not an error");
}

#[tokio::test]
async fn a_batch_delete_still_deletes_what_is_there() {
    let s = Store::memory().await.expect("open");
    write(&s, "ws", "role", "operator", &json!({ "caps": [] }))
        .await
        .expect("seed");
    let deletes = vec![DeleteBatch {
        table: "role",
        id: "operator",
    }];
    write_batch(&s, "ws", &[], &deletes).await.expect("delete");
    assert_eq!(
        read(&s, "ws", "role", "operator").await.expect("read"),
        None,
        "the row must actually be gone"
    );
}

/// The mixed cascade `roles.delete` actually performs: tombstone the assignees AND drop the role,
/// in one transaction, where the role table may not exist in this workspace at all.
#[tokio::test]
async fn a_mixed_batch_commits_when_the_delete_target_is_absent() {
    let s = Store::memory().await.expect("open");
    let tombstone = json!({ "subject": "user:bob", "cap": "__revoked__" });
    let upserts = vec![UpsertBatch {
        table: "grant",
        id: "g1",
        value: &tombstone,
    }];
    let deletes = vec![DeleteBatch {
        table: "role",
        id: "operator",
    }];
    write_batch(&s, "fresh", &upserts, &deletes)
        .await
        .expect("the cascade must commit");
    assert!(
        read(&s, "fresh", "grant", "g1")
            .await
            .expect("read")
            .is_some(),
        "the upsert half must have landed"
    );
}

/// The interpolated name is charset-checked, because `DEFINE TABLE` takes a literal.
#[tokio::test]
async fn an_illegal_table_identifier_is_refused_not_quoted() {
    let s = Store::memory().await.expect("open");
    let deletes = vec![DeleteBatch {
        table: "role; DEFINE USER hax ON ROOT PASSWORD 'x' ROLES OWNER",
        id: "operator",
    }];
    assert!(
        write_batch(&s, "ws", &[], &deletes).await.is_err(),
        "a table name that is not a bare identifier must be refused"
    );
}
