//! [`write_batch`] — atomically apply a batch of upserts AND deletes to workspace `ws` in ONE
//! SurrealDB transaction (access-console scope: the `roles.delete` cascade — un-assign a role from
//! N subjects AND drop the role record — must be all-or-nothing). This is the generalization
//! [`write_tx`](crate::write_tx) is a 2-upsert special case of: N upserts + M deletes behind one
//! `BEGIN … COMMIT TRANSACTION`, so either every change lands or none does.
//!
//! Every statement operates in the SAME workspace namespace (selected from `ws` first), so the hard
//! wall holds for the batch exactly as for a single write (README §7). Caller is expected to have
//! passed `caps::check` — this is the raw verb, not the authorization point. Like [`write`], each
//! upsert bumps its record's monotonic `rev`; deletes do not (no record left to carry one).
//!
//! **Bounded** on purpose: a batch is for the bounded, same-logical-tx writes a verb performs
//! (e.g. "un-assign this role from its ≤ N assignees and delete it"), not an unbounded bulk load.
//! The cap is enforced here so a runaway caller fails fast instead of holding a long transaction.

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::FIRST_REV;
use crate::taint::mark_store_written;

/// The maximum number of statements (upserts + deletes) one batch may carry. Bounds the
/// transaction's length so a runaway caller cannot hold an open tx for an unbounded set.
pub const MAX_BATCH: usize = 256;

/// One upsert in a batch: its table, id, and host JSON value.
pub struct UpsertBatch<'a> {
    pub table: &'a str,
    pub id: &'a str,
    pub value: &'a Value,
}

/// One delete in a batch: its table and id.
pub struct DeleteBatch<'a> {
    pub table: &'a str,
    pub id: &'a str,
}

/// Atomically upsert `upserts` and delete `deletes` in workspace `ws`, in one transaction. Either
/// every change commits or none does (SurrealDB rolls the transaction back as a unit on any error).
/// `StoreError::Decode` (mis)used for an over-limit or empty batch (a no-op batch is a caller bug).
pub async fn write_batch(
    store: &Store,
    ws: &str,
    upserts: &[UpsertBatch<'_>],
    deletes: &[DeleteBatch<'_>],
) -> Result<(), StoreError> {
    let total = upserts.len() + deletes.len();
    if total == 0 {
        return Err(StoreError::Decode("write_batch: empty batch".into()));
    }
    if total > MAX_BATCH {
        return Err(StoreError::Decode(format!(
            "write_batch: {total} statements exceed the {MAX_BATCH} cap"
        )));
    }

    let db = store.use_ws(ws).await?;
    // Build one BEGIN/COMMIT with the upserts then the deletes. Each upsert bumps its record's
    // `rev` server-side (same expression `write` uses); deletes carry no rev. All params are bound
    // by position; the query text is assembled from caller-controlled `table`/`id` only via
    // `type::thing($tbN, $idN)` binding, so no caller string reaches the query text.
    let mut sql = String::from("BEGIN TRANSACTION;");
    for i in 0..upserts.len() {
        sql.push_str(&format!(
            " UPSERT type::thing($ut{i}, $ui{i}) CONTENT {{ data: $ud{i}, \
             rev: (type::thing($ut{i}, $ui{i}).rev ?? ($first - 1)) + 1 }} RETURN NONE;"
        ));
    }
    for j in 0..deletes.len() {
        sql.push_str(&format!(" DELETE type::thing($dt{j}, $dj{j}) RETURN NONE;"));
    }
    sql.push_str(" COMMIT TRANSACTION;");

    let mut q = db.query(sql).bind(("first", FIRST_REV));
    for (i, u) in upserts.iter().enumerate() {
        q = q
            .bind((format!("ut{i}"), u.table.to_string()))
            .bind((format!("ui{i}"), u.id.to_string()))
            .bind((format!("ud{i}"), u.value.clone()));
    }
    for (j, d) in deletes.iter().enumerate() {
        q = q
            .bind((format!("dt{j}"), d.table.to_string()))
            .bind((format!("dj{j}"), d.id.to_string()));
    }
    q.await?.check()?;
    // A multi-record transaction also mutates the store (no-op outside a dispatch taint scope).
    mark_store_written();
    Ok(())
}
