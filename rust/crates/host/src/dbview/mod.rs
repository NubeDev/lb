//! The DB-browser service — the host's capability chokepoint for the admin, **read-only** raw store
//! lens (data-console scope, README §3). Wraps the generic `lb_store` reads (`tables`/`scan`/`graph`)
//! with the gate (capability-first §3.5, isolation-first §3.6) so a non-SQL workspace admin can see
//! "what's actually in here" without writing a query — a paged row grid + a react-flow relation graph.
//!
//! **The headline decision:** these verbs deliberately relax the per-record membership gate (gate 3)
//! that typed verbs like `get_doc` enforce — a raw scan answers "every record in the workspace". So
//! they are **admin-only** (`mcp:store.tables/scan/graph:call` granted to the workspace-admin role,
//! NOT `member_caps`) and **read-only** (no write verbs by design — edits go through the domain
//! verbs, never the raw grid). Two gates still hold hard: the workspace wall and the capability.
//!
//! The verbs (one per file, FILE-LAYOUT):
//!   - `store.tables` ([`store_tables_view`]) — list tables + row counts (the picker).
//!   - `store.scan` ([`store_scan_view`]) — a bounded, id-cursor-paged page of raw rows (the grid).
//!   - `store.graph` ([`store_graph_view`]) — bounded nodes + relation edges (react-flow).
//!   - the MCP bridge ([`call_dbview_tool`]) — the one MCP contract over all three.

mod authorize;
mod error;
mod graph;
mod scan;
mod tables;
mod tool;

pub use authorize::authorize_dbview;
pub use error::DbViewError;
pub use graph::store_graph_view;
pub use scan::store_scan_view;
pub use tables::store_tables_view;
pub use tool::call_dbview_tool;

// Re-export the wire shapes so host callers / the gateway / tests use one type set.
pub use lb_store::{Graph, GraphEdge, GraphNode, Page, Row, TableCount};
