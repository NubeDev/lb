//! The generic store-**mutation** surface — the write half of the "direct SurrealDB" contract, the
//! companion to the read-only `store_query` (widget-builder Slice A). Two host-native MCP verbs over
//! the embedded store, each gated per-table and workspace-walled:
//!
//!   - `store.write(table, id, value) -> { table, id }` ([`store_write_run`]) — UPSERT a JSON record
//!     at `table:id` in the caller's workspace namespace (via [`lb_store::write`], which bumps the
//!     record's monotonic `rev`).
//!   - `store.delete(table, id) -> { table, id }` ([`store_delete_run`]) — erase `table:id` (idempotent).
//!   - the MCP bridge ([`call_store_mutate_tool`]) — the one MCP contract over both.
//!
//! **Two gates, mirroring the read side + `rules_save`:** the outer `mcp:store.<verb>:call` gate runs
//! at the dispatcher (workspace-first); this module re-runs the **per-table store-surface** gate —
//! `store:<table>:write` — via the shared `caps::check` chokepoint (defense in depth, and it is the
//! grant that actually scopes *which* tables a holder may touch). A native sidecar reaches this over
//! its `SidecarClient` callback exactly as it reaches `store.query` — so an extension gets a generic,
//! caps-scoped write path to its OWN table (requesting `store:<its-table>:write` in its manifest)
//! without any host code knowing that table exists.
//!
//! Delete is gated under the SAME `write` action as `rules_delete` — a delete is a mutation of the
//! table, and the grammar has no distinct `delete` action; a holder of `store:<table>:write` may
//! both upsert and erase within that table. (A finer split would be a grammar change, deferred until
//! a caller needs erase-without-write.)

mod authorize;
mod error;
mod run;
mod tool;

pub use authorize::authorize_store_mutate;
pub use error::StoreMutateError;
pub use run::{store_delete_run, store_write_run};
pub use tool::call_store_mutate_tool;
