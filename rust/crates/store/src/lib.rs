//! The datastore — embedded SurrealDB, the one source of truth on every node (README §6.1).
//!
//! Tenancy mapping (§7): **workspace = SurrealDB namespace**. A [`Store`] handle is opened
//! once; each operation is scoped to a workspace, which selects the namespace before the
//! query runs. That makes workspace isolation *structural* at the store layer — a query for
//! workspace A physically cannot read namespace B's records.
//!
//! State only (§3.3): the store holds state; motion is the bus's job. No pub/sub here.

mod boot_guard;
mod boot_pass;
mod capped;
mod compact;
mod conflict;
mod create;
mod delete;
mod graph;
mod increment;
mod last_pass;
mod list;
mod meminfo;
mod open;
mod read;
mod read_versioned;
mod record;
pub mod reserved;
mod scan;
mod scan_all;
mod snapshot_guard;
mod status;
mod tables;
mod taint;
mod write;
mod write_batch;
mod write_journaled;
mod write_locked;
mod write_tx;

pub use boot_guard::{
    boot_compaction_skip, is_productive, open_would_not_fit, BOOT_COMPACT_MEM_RATIO,
    OPEN_GUARD_MEM_RATIO, PRODUCTIVE_RECLAIM_RATIO, REGROWTH_RERUN_RATIO,
};
pub use capped::{capped_insert, new_ulid, ulid_timestamp_ms};
pub use compact::{compact, CompactionRecord};
pub use create::create;
pub use delete::delete;
pub use graph::{graph, Edge as GraphEdge, Graph, Node as GraphNode, MAX_FANOUT, MAX_SEED};
pub use increment::increment;
pub use last_pass::last_persisted_compaction;
pub use list::list;
pub use meminfo::available_ram_bytes;
pub use open::{OpenOptions, Store, StoreError};
pub use read::read;
pub use read_versioned::read_versioned;
pub use record::{Versioned, FIRST_REV};
pub use reserved::{is_reserved, RESERVED_TABLES};
pub use scan::{scan, Page, Row, MAX_SCAN_LIMIT};
pub use scan_all::scan_all;
pub use snapshot_guard::{snapshot_safety, SnapshotRefusal};
pub use status::{status, StoreStatus};
pub use tables::{tables, TableCount};
pub use taint::{
    mark_outbox_reached, mark_store_written, outbox_was_reached, store_was_written, taint_scope,
    TaintVerdict,
};
pub use write::write;
pub use write_batch::{write_batch, DeleteBatch, UpsertBatch, MAX_BATCH};
pub use write_journaled::{write_journaled, JournalWrite};
pub use write_locked::write_locked;
pub use write_tx::{write_tx, Upsert};
