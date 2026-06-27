//! The read-only SQL service — the host's capability chokepoint for the "direct SurrealDB" widget
//! source (widget-builder follow-up Slice A). Two host-native MCP verbs over the embedded store, each
//! gated and workspace-walled:
//!
//!   - `store.query(sql, vars?) -> { columns, rows }` ([`store_query_run`]) — a **parse-allowlisted,
//!     bounded, read-only** SurrealQL run. The load-bearing read-only gate is `parse.rs`: PARSE the
//!     statement and allow only a single `SELECT`/`INFO`/`SHOW` — never a substring check, never a
//!     write/schema/`USE`. Runs inside the caller's workspace namespace (from the token), capped at
//!     `MAX_QUERY_ROWS` rows / `QUERY_TIMEOUT_SECS` seconds.
//!   - `store.schema() -> { tables: [{ name, columns: [{name,type}] }] }` ([`store_schema_read`]) —
//!     the workspace schema from SurrealDB introspection, feeding the visual SQL builder (Slice C).
//!   - the MCP bridge ([`call_store_query_tool`]) — the one MCP contract over both.
//!
//! It is **just a tool on the widget bridge**: a cell calls it only if `mcp:store.query:call` (or
//! `:schema:`) ∈ `cell.tools ∩ install-grant`, re-checked at the host per call — the same leash as
//! every other tool, no special widget path. The parse gate + the workspace wall + the row cap are
//! the boundary; the visual builder (Slice C) is convenience above it.

mod authorize;
mod error;
mod model;
mod parse;
mod run;
mod schema;
mod tool;

pub use authorize::authorize_store_query;
pub use error::StoreQueryError;
pub use model::{
    QueryResult, Schema, SchemaColumn, SchemaTable, MAX_QUERY_ROWS, QUERY_TIMEOUT_SECS,
};
pub use parse::ensure_read_only;
pub use run::store_query_run;
pub use schema::store_schema_read;
pub use tool::call_store_query_tool;
