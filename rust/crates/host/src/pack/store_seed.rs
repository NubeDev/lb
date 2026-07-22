//! Seed a pack's `store`-engine datasource — the entity rows land as SurrealDB records in the ONE
//! application store, NOT a node-local sqlite file (`pack-store-datasource-scope.md`). The companion
//! to `sqlite.rs`: where that materializes a private `.db`, this UPSERTs each declared row at
//! `{table}:{pk}` through the very `store.write` verb every other workspace record rides — so a pack's
//! `site`/`meter`/`point` rows are Data-browser-visible, graph-linkable via `rel`, and caps-scopable,
//! the whole point of the scope.
//!
//! ## Why this file exists, and the line it holds
//!
//! A `sqlite` pack ships `seed.sql` (INSERTs run in-process). A `store` pack ships `seed_rows`
//! (structured JSON, `{table: [rows]}`) because the store takes structured values, never caller SQL —
//! it mirrors `federation.write`'s no-SQL contract (O-1). There is no new storage engine and no new
//! capability here: `store.write` already exists, is gated per-table (`store:<table>:write`), and is
//! workspace-walled by the caller's token. This is the same seam the EMS extension already writes its
//! sites/meters through — packs now do the same.
//!
//! ## Seed ownership (run-once), enforced above this file
//!
//! Like the sqlite seed, the store seed is starting data applied ONCE: `apply.rs` calls [`seed_rows`]
//! only on the FIRST apply. A re-apply/upgrade never re-clobbers — so an operator who edits a seeded
//! `site` record (or adds their own) keeps it. This is MORE natural in the store than in sqlite: each
//! record already carries a monotonic `rev`, and we never touch a table's existing rows on a re-apply.
//!
//! ## No privileged path (rule 10 / pack-core caps)
//!
//! Each row is written through [`crate::store_write_run`] under the CALLER's principal, so the seed
//! re-checks `store:<table>:write` per row exactly as a hand `store.write` would. A pack seeding
//! `site` needs `store:site:write` on the applier; without it the object is `denied` and the receipt
//! records an honest partial — a pack grants no smuggled write.

use lb_auth::Principal;
use serde_json::Value;

use super::error::PackError;
use crate::boot::Node;

/// UPSERT every declared row of every `seed_rows` table into the workspace's store, keyed by the
/// binding's `pk`. `pk_for(table)` yields the primary-key COLUMN a downstream editor writes as the
/// record id — resolved from the entity binding that names this table. Returns the count seeded.
///
/// The record id IS the pk: a row `{ "id": "site-001", "name": "…" }` bound `pk: id` writes
/// `site:site-001` with the whole row as `value` (the store wraps it in its `{ data }` envelope). A
/// row missing its pk field is a hard [`PackError::BadInput`] naming the table — a store record
/// cannot exist without an id, and a silent skip would seed a pack that is not the one authored.
pub async fn seed_rows<F>(
    node: &Node,
    principal: &Principal,
    ws: &str,
    seed: &std::collections::BTreeMap<String, Vec<Value>>,
    pk_for: F,
) -> Result<usize, PackError>
where
    F: Fn(&str) -> Option<String>,
{
    let mut written = 0usize;
    for (table, rows) in seed {
        // The pk column for this table comes from the entity binding. A `seed_rows` table with no
        // bound entity (or a binding with no `pk`) has no id column to key on — refuse loudly rather
        // than invent one.
        let pk = pk_for(table).ok_or_else(|| {
            PackError::BadInput(format!(
                "store seed table '{table}' has no bound entity with a `pk` — a store record needs \
                 an id column; bind the entity (table/pk) or drop the seed rows"
            ))
        })?;
        // Seed-ownership, per table: seed ONLY an empty store table. A table the migration already
        // filled with the operator's live rows (or that a concurrent path populated) is the
        // operator's — the seed never overwrites it. On a fresh workspace every table is empty, so
        // this is a no-op; combined with the first-apply guard above, the seed is truly run-once.
        if !store_table_empty(node, ws, table).await? {
            continue;
        }
        for row in rows {
            let id = row
                .get(&pk)
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| row.get(&pk).and_then(|v| match v {
                    // A numeric pk is accepted (rendered as its canonical string id).
                    Value::Number(n) => Some(n.to_string()),
                    _ => None,
                }))
                .ok_or_else(|| {
                    PackError::BadInput(format!(
                        "store seed table '{table}': a row is missing its pk field '{pk}' (or it is \
                         not a string/number) — every store record needs an id: {row}"
                    ))
                })?;
            crate::store_write_run(&node.store, principal, ws, table, &id, row)
                .await
                .map_err(|e| store_err(table, &id, e))?;
            written += 1;
        }
    }
    Ok(written)
}

/// Map a `store.write` failure onto the pack error vocabulary. A capability denial keeps its
/// `Denied` shape so `apply.rs` records the object as `denied` (the recoverable partial); anything
/// else is a hard failure naming the record.
fn store_err(table: &str, id: &str, e: crate::StoreMutateError) -> PackError {
    match e {
        crate::StoreMutateError::Denied => PackError::Denied,
        other => PackError::Internal(format!("seeding {table}:{id}: {other}")),
    }
}

/// MIGRATE the operator's live entity rows from a PRIOR sqlite datasource into the store, before the
/// seed runs (`pack-store-datasource-scope.md` §Migration). For each store-bound table that EXISTS in
/// the old sqlite db, read every LIVE row (not the pack's seed — the operator may have edited/added
/// rows) and UPSERT it at `{table}:{pk}` in the store — but ONLY when the store table is currently
/// EMPTY, so a re-run never clobbers store data and a workspace that never had the sqlite era is a
/// no-op. Returns the per-table migrated counts (for the loud upgrade note).
///
/// Safety (the sharp requirement): a failed migration leaves the sqlite file IN PLACE (this function
/// never deletes it — dropping the old source is a separate, later act), so a half-move cannot strand
/// the operator's data. A missing sqlite file, or a table the old db lacks, is simply skipped (nothing
/// to carry). `tables` pairs each store-bound table with its pk column (from the entity bindings).
pub async fn migrate_sqlite_entities(
    node: &Node,
    principal: &Principal,
    ws: &str,
    sqlite_path: &std::path::Path,
    tables: &[(String, String)],
) -> Result<Vec<(String, usize)>, PackError> {
    if !sqlite_path.is_file() {
        return Ok(Vec::new()); // no prior sqlite era in this workspace — nothing to migrate.
    }
    let conn = rusqlite::Connection::open(sqlite_path)
        .map_err(|e| PackError::Internal(format!("opening prior sqlite db for migration: {e}")))?;

    let mut migrated = Vec::new();
    for (table, pk) in tables {
        // Skip a table the old db doesn't have (this store entity is new in this version).
        let live = match read_sqlite_rows(&conn, table) {
            Ok(rows) => rows,
            Err(_) => continue,
        };
        if live.is_empty() {
            continue;
        }
        // Only migrate into an EMPTY store table — never clobber rows the store already holds.
        if !store_table_empty(node, ws, table).await? {
            continue;
        }
        let mut n = 0usize;
        for row in &live {
            let id = row
                .get(pk)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| {
                    PackError::BadInput(format!(
                        "migrating '{table}': a live sqlite row is missing its pk '{pk}': {row}"
                    ))
                })?;
            crate::store_write_run(&node.store, principal, ws, table, &id, row)
                .await
                .map_err(|e| store_err(table, &id, e))?;
            n += 1;
        }
        migrated.push((table.clone(), n));
    }
    Ok(migrated)
}

/// Read every row of a sqlite `table` as JSON objects keyed by column. Uses `PRAGMA table_info` for
/// the column names, then `SELECT *` — so the migrated store record mirrors the sqlite row exactly.
/// A sqlite value maps to JSON: NULL→null, INTEGER→number, REAL→number, TEXT→string, BLOB→skipped
/// (entity tables hold no blobs). An error (a missing table) propagates so the caller skips it.
fn read_sqlite_rows(
    conn: &rusqlite::Connection,
    table: &str,
) -> Result<Vec<Value>, rusqlite::Error> {
    let cols: Vec<String> = {
        let mut stmt = conn.prepare(&format!(
            "PRAGMA table_info(\"{}\")",
            table.replace('"', "\"\"")
        ))?;
        let names = stmt.query_map([], |r| r.get::<_, String>(1))?;
        names.filter_map(Result::ok).collect()
    };
    if cols.is_empty() {
        return Err(rusqlite::Error::QueryReturnedNoRows); // no such table → caller skips.
    }
    let mut stmt = conn.prepare(&format!("SELECT * FROM \"{}\"", table.replace('"', "\"\"")))?;
    let rows = stmt.query_map([], |r| {
        let mut obj = serde_json::Map::new();
        for (i, col) in cols.iter().enumerate() {
            let v = match r.get_ref(i)? {
                rusqlite::types::ValueRef::Null => Value::Null,
                rusqlite::types::ValueRef::Integer(n) => Value::from(n),
                rusqlite::types::ValueRef::Real(f) => Value::from(f),
                rusqlite::types::ValueRef::Text(t) => {
                    Value::from(String::from_utf8_lossy(t).into_owned())
                }
                rusqlite::types::ValueRef::Blob(_) => continue,
            };
            obj.insert(col.clone(), v);
        }
        Ok(Value::Object(obj))
    })?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// Is the store `table` empty in `ws`? A migration only writes into an empty table (never clobber).
async fn store_table_empty(node: &Node, ws: &str, table: &str) -> Result<bool, PackError> {
    let mut resp = node
        .store
        .query_ws(
            ws,
            "SELECT count() AS n FROM type::table($tb) GROUP ALL",
            vec![("tb".into(), Value::from(table))],
        )
        .await
        .map_err(|e| PackError::Internal(format!("counting store table '{table}': {e}")))?;
    let counts: Vec<i64> = resp.take((0, "n")).unwrap_or_default();
    Ok(counts.first().copied().unwrap_or(0) == 0)
}
