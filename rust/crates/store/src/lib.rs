//! The datastore — embedded SurrealDB, the one source of truth on every node (README §6.1).
//!
//! Tenancy mapping (§7): **workspace = SurrealDB namespace**. A [`Store`] handle is opened
//! once; each operation is scoped to a workspace, which selects the namespace before the
//! query runs. That makes workspace isolation *structural* at the store layer — a query for
//! workspace A physically cannot read namespace B's records.
//!
//! State only (§3.3): the store holds state; motion is the bus's job. No pub/sub here.

mod capped;
mod create;
mod delete;
mod graph;
mod list;
mod open;
mod read;
mod read_versioned;
mod record;
mod scan;
mod tables;
mod taint;
mod write;
mod write_batch;
mod write_journaled;
mod write_tx;

pub use capped::{capped_insert, new_ulid};
pub use create::create;
pub use delete::delete;
pub use graph::{graph, Edge as GraphEdge, Graph, Node as GraphNode, MAX_FANOUT, MAX_SEED};
pub use list::list;
pub use open::{Store, StoreError};
pub use read::read;
pub use read_versioned::read_versioned;
pub use record::{Versioned, FIRST_REV};
pub use scan::{scan, Page, Row, MAX_SCAN_LIMIT};
pub use tables::{tables, TableCount};
pub use taint::{
    mark_outbox_reached, mark_store_written, outbox_was_reached, store_was_written, taint_scope,
    TaintVerdict,
};
pub use write::write;
pub use write_batch::{write_batch, DeleteBatch, UpsertBatch, MAX_BATCH};
pub use write_journaled::{write_journaled, JournalWrite};
pub use write_tx::{write_tx, Upsert};
