//! The datastore — embedded SurrealDB, the one source of truth on every node (README §6.1).
//!
//! Tenancy mapping (§7): **workspace = SurrealDB namespace**. A [`Store`] handle is opened
//! once; each operation is scoped to a workspace, which selects the namespace before the
//! query runs. That makes workspace isolation *structural* at the store layer — a query for
//! workspace A physically cannot read namespace B's records.
//!
//! State only (§3.3): the store holds state; motion is the bus's job. No pub/sub here.

mod graph;
mod list;
mod open;
mod read;
mod record;
mod scan;
mod tables;
mod write;
mod write_tx;

pub use graph::{graph, Edge as GraphEdge, Graph, Node as GraphNode, MAX_FANOUT, MAX_SEED};
pub use list::list;
pub use open::{Store, StoreError};
pub use read::read;
pub use scan::{scan, Page, Row, MAX_SCAN_LIMIT};
pub use tables::{tables, TableCount};
pub use write::write;
pub use write_tx::{write_tx, Upsert};
