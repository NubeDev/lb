//! The wire shapes for the read-only SQL surface (`store.query` / `store.schema`) — widget-builder
//! follow-up Slice A. A `QueryResult` is the column-oriented result the dashboard's table/chart views
//! consume unchanged (rows are `serde_json::Value` objects keyed by column). A `Schema` is the
//! workspace's tables + their columns, feeding the visual SQL builder (Slice C).
//!
//! Both are derived **inside the caller's workspace namespace** (the wall, set host-side from the
//! token) — never a workspace named in the SQL. One responsibility per file (FILE-LAYOUT): these are
//! only the data; the gate lives in `authorize.rs`, the parse-allowlist in `parse.rs`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The default + ceiling on rows a single `store.query` returns. The handler injects/enforces a
/// `LIMIT` no larger than this — an unbounded analytical scan is a job, not this synchronous verb
/// (widget-builder scope, "Bounded"). 10k is the interactive-read ceiling the scope names.
pub const MAX_QUERY_ROWS: usize = 10_000;

/// The statement timeout (seconds) applied to a `store.query` run — the scope's 5s interactive bound.
/// Enforced through SurrealQL's `TIMEOUT` clause appended to the read so a pathological query cannot
/// block the node.
pub const QUERY_TIMEOUT_SECS: u64 = 5;

/// The result of a `store.query` — column names (in first-seen order across the returned rows) plus
/// the rows as JSON objects. The dashboard's `table`/`chart`/`stat`/`plot`/`template` views render
/// `rows` directly; `columns` drives the table header + the chart's x/y column picker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Value>,
}

/// One column of a table as `store.schema` reports it: its name and a best-effort type string (from
/// the `DEFINE FIELD` `TYPE`, or `"any"` when the schema is schemaless / the field is untyped).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// One table in the workspace and the columns the visual builder offers for it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SchemaTable {
    pub name: String,
    pub columns: Vec<SchemaColumn>,
}

/// The workspace's schema — every table + its columns, sorted by table name. Empty if the namespace
/// has no tables; never another workspace's tables (the wall).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Schema {
    pub tables: Vec<SchemaTable>,
}
