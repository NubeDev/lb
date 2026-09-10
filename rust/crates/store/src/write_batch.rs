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

/// `[A-Za-z_][A-Za-z0-9_]*` — the only shape allowed to be interpolated into the query text (see
/// the `DEFINE TABLE` note in [`write_batch`]). Deliberately narrower than SurrealDB's own quoted
/// identifiers: everything lb writes is a bare identifier, so a name that is not one is a caller
/// bug, and refusing it keeps the "no caller string in the query text" rule true in spirit.
fn is_bare_ident(s: &str) -> bool {
    let mut c = s.chars();
    matches!(c.next(), Some(f) if f.is_ascii_alphabetic() || f == '_')
        && c.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
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

    // Build one BEGIN/COMMIT with the upserts then the deletes. Each upsert bumps its record's
    // `rev` server-side (same expression `write` uses); deletes carry no rev. All params are bound
    // by position; the query text is assembled from caller-controlled `table`/`id` only via
    // `type::record($tbN, $idN)` binding, so no caller string reaches the query text — with the one
    // exception below, which is why that exception is charset-checked first.
    let mut sql = String::from("BEGIN TRANSACTION;");

    // SurrealDB 3 raises `NotFoundError::Table` for a DELETE against a table that was never
    // written; SurrealDB 2 answered with no rows. `query_ws`'s shim drops that error for a bare
    // statement, but it cannot help INSIDE a transaction: the failed statement aborts the whole
    // transaction, and the COMMIT then fails with a different error ("Cannot COMMIT: the
    // transaction was aborted due to a prior error") that names no table at all.
    //
    // So the delete targets are defined first, idempotently — the same thing ingest, prefs, agent
    // config, tags and undo do for their own tables. Deleting from a table that now exists and
    // holds nothing is a clean no-op, which is exactly what the caller means. Without this, a
    // cascade that legitimately finds nothing to delete (a `roles.delete` for a role that lives in
    // ANOTHER workspace) failed the whole request instead of reporting `affected: 0`.
    //
    // `DEFINE TABLE` takes a literal name, not a `$param`, so these table names are interpolated.
    // That is the one place a caller string reaches the query text, so the charset is checked shut
    // first: a name that is not a bare identifier is refused rather than quoted-and-hoped.
    let mut defined: Vec<&str> = Vec::new();
    for d in deletes {
        if defined.contains(&d.table) {
            continue;
        }
        if !is_bare_ident(d.table) {
            return Err(StoreError::Decode(format!(
                "write_batch: {:?} is not a legal table identifier",
                d.table
            )));
        }
        sql.push_str(&format!(" DEFINE TABLE IF NOT EXISTS {};", d.table));
        defined.push(d.table);
    }
    for i in 0..upserts.len() {
        sql.push_str(&format!(
            " UPSERT type::record($ut{i}, $ui{i}) CONTENT {{ data: $ud{i}, \
             rev: (type::record($ut{i}, $ui{i}).rev ?? ($first - 1)) + 1 }} RETURN NONE;"
        ));
    }
    for j in 0..deletes.len() {
        sql.push_str(&format!(
            " DELETE type::record($dt{j}, $dj{j}) RETURN NONE;"
        ));
    }
    sql.push_str(" COMMIT TRANSACTION;");

    let mut bindings: Vec<(String, Value)> = vec![("first".into(), Value::from(FIRST_REV))];
    for (i, u) in upserts.iter().enumerate() {
        bindings.push((format!("ut{i}"), Value::String(u.table.to_string())));
        bindings.push((format!("ui{i}"), Value::String(u.id.to_string())));
        bindings.push((format!("ud{i}"), u.value.clone()));
    }
    for (j, d) in deletes.iter().enumerate() {
        bindings.push((format!("dt{j}"), Value::String(d.table.to_string())));
        bindings.push((format!("dj{j}"), Value::String(d.id.to_string())));
    }
    store.query_ws(ws, &sql, bindings).await?;
    // A multi-record transaction also mutates the store (no-op outside a dispatch taint scope).
    mark_store_written();
    Ok(())
}
