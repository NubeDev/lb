//! `store.schema` — the read-only workspace schema (widget-builder Slice A → feeds Slice C's visual
//! SQL builder). Authorize (gate 1+2), then derive `{ tables: [{ name, columns: [{name,type}] }] }`
//! from SurrealDB introspection inside the **caller's workspace namespace** (the wall, from the
//! token). `INFO FOR DB` lists the tables; `INFO FOR TABLE` gives each table's defined fields + their
//! `TYPE`. Our store is largely **schemaless** (records are a `{ data: … }` envelope), so when a
//! table has no `DEFINE FIELD`s we fall back to sampling one row's keys — the columns a builder
//! dropdown needs, typed `"any"`.
//!
//! Read-only and bounded: at most one `INFO FOR TABLE` + one 1-row sample per table; a ws-B caller
//! sees only ws-B's tables (structurally — `query_ws` selects B's namespace first).

use std::collections::BTreeMap;

use lb_auth::Principal;
use lb_store::{Store, StoreError};
use serde_json::Value;

use super::authorize::authorize_store_query;
use super::error::StoreQueryError;
use super::model::{Schema, SchemaColumn, SchemaTable};

/// Read the workspace's schema for the visual SQL builder. Gated `mcp:store.schema:call`,
/// workspace-walled. Tables sorted by name; columns in first-seen order.
pub async fn store_schema_read(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Schema, StoreQueryError> {
    authorize_store_query(principal, ws, "store.schema")?;

    let mut table_names = db_tables(store, ws).await?;
    table_names.sort();

    let mut tables = Vec::with_capacity(table_names.len());
    for name in table_names {
        let columns = table_columns(store, ws, &name).await?;
        tables.push(SchemaTable { name, columns });
    }
    Ok(Schema { tables })
}

/// The table names defined in `ws` (the keys of `INFO FOR DB`'s `tables` map).
async fn db_tables(store: &Store, ws: &str) -> Result<Vec<String>, StoreQueryError> {
    let mut resp = store.query_ws(ws, "INFO FOR DB", vec![]).await?;
    let info: Option<DbInfo> = resp.take(0).map_err(decode)?;
    Ok(info
        .map(|i| i.tables.into_keys().collect())
        .unwrap_or_default())
}

/// The columns of `table`: its `DEFINE FIELD`s (name + `TYPE`) if schemafull, else the keys of one
/// sampled row (typed `"any"`) for our schemaless records.
async fn table_columns(
    store: &Store,
    ws: &str,
    table: &str,
) -> Result<Vec<SchemaColumn>, StoreQueryError> {
    // `INFO FOR TABLE <ident>` takes a literal table identifier (not a bound param / `type::table`).
    // The name came from `INFO FOR DB`'s own table map — trusted introspection output, not user
    // input — but we still backtick-quote it so an unusual-but-valid table name is a single literal
    // identifier and can never be more than one. Its `fields` map is field-name → DEFINE FIELD text.
    let mut resp = store
        .query_ws(
            ws,
            &format!("INFO FOR TABLE `{}`", escape_ident(table)),
            vec![],
        )
        .await?;
    let info: Option<TableInfo> = resp.take(0).map_err(decode)?;
    let fields = info.map(|i| i.fields).unwrap_or_default();

    if !fields.is_empty() {
        let mut cols: Vec<SchemaColumn> = fields
            .into_iter()
            .map(|(name, define)| SchemaColumn {
                ty: type_of_define(&define),
                name,
            })
            .collect();
        cols.sort_by(|a, b| a.name.cmp(&b.name));
        return Ok(cols);
    }

    // Schemaless: sample one row and report its keys (typed `any`). Records are stored under a
    // `{ data: … }` envelope, so unwrap it to surface the author's own columns, not the wrapper.
    sample_columns(store, ws, table).await
}

/// Columns from one sampled row of a schemaless table — its object keys, in first-seen order, typed
/// `"any"`. No rows → no columns (an empty table offers none).
async fn sample_columns(
    store: &Store,
    ws: &str,
    table: &str,
) -> Result<Vec<SchemaColumn>, StoreQueryError> {
    let mut resp = store
        .query_ws(
            ws,
            "SELECT * OMIT id, in, out FROM type::table($tb) LIMIT 1",
            vec![("tb".into(), Value::String(table.to_string()))],
        )
        .await?;
    let rows: Vec<Value> = resp.take(0).map_err(decode)?;
    let obj = match rows.into_iter().next() {
        Some(Value::Object(mut o)) => match o.remove("data") {
            // `lb_store::write` wraps the value in `{ data: … }`; surface the inner columns.
            Some(Value::Object(inner)) => inner,
            _ => o,
        },
        _ => return Ok(Vec::new()),
    };
    Ok(obj
        .keys()
        .map(|name| SchemaColumn {
            name: name.clone(),
            ty: "any".into(),
        })
        .collect())
}

/// Pull the declared `TYPE` out of a `DEFINE FIELD … TYPE <t> …` statement; `"any"` if none.
fn type_of_define(define: &str) -> String {
    match define.split_once(" TYPE ") {
        Some((_, rest)) => rest
            .split_whitespace()
            .next()
            .unwrap_or("any")
            .trim_end_matches(['(', ',', ';'])
            .to_string(),
        None => "any".into(),
    }
}

/// Escape a backtick-quoted identifier (a table name from introspection): double any backtick so the
/// name stays a single literal identifier. Defensive — the names come from the store's own `INFO FOR
/// DB`, but quoting keeps the one-identifier invariant explicit.
fn escape_ident(name: &str) -> String {
    name.replace('`', "``")
}

fn decode(e: surrealdb::Error) -> StoreQueryError {
    StoreQueryError::Store(StoreError::Decode(e.to_string()))
}

/// The `tables` slice of `INFO FOR DB` (table-name → DEFINE text); other fields ignored.
#[derive(serde::Deserialize)]
struct DbInfo {
    #[serde(default)]
    tables: BTreeMap<String, Value>,
}

/// The `fields` slice of `INFO FOR TABLE` (field-name → DEFINE FIELD text); other fields ignored.
#[derive(serde::Deserialize)]
struct TableInfo {
    #[serde(default)]
    fields: BTreeMap<String, String>,
}
