//! `store.graph` — authorize (admin cap), then read a depth/fan-out-bounded slice of the workspace
//! graph (nodes + relation edges) for react-flow. The raw read is `lb_store::graph`; this layer adds
//! the gate AND supplies the relation tables to walk (data-console scope). Read-only.
//!
//! The relation tables are named **here**, not in `lb_store` (which stays generic): today the one
//! real relation-edge record set is the tag layer's `tagged` (`lb_tags::TAGGED_TABLE` —
//! `entity -> tagged -> tag`). As more relations ship as edge records (team→member, doc→channel),
//! they are added to this list — never synthesised from a join.

use lb_auth::Principal;
use lb_store::{graph as store_graph, Graph, Store};
use lb_tags::TAGGED_TABLE;

use super::authorize::authorize_dbview;
use super::error::DbViewError;

/// The relation-edge tables the graph view walks. Each is a real `RELATE in -> edge -> out` record
/// set; the first cut draws the tag graph. Extend as more edge records ship.
const EDGE_TABLES: &[&str] = &[TAGGED_TABLE];

/// Build a bounded graph slice in `ws` seeded from a `table` and/or a single record `id`. Gated by
/// `mcp:store.graph:call` (admin-only). Depth/fan-out bounded by `lb_store`. Namespace-scoped.
pub async fn store_graph_view(
    store: &Store,
    principal: &Principal,
    ws: &str,
    table: Option<&str>,
    id: Option<&str>,
    depth: u32,
) -> Result<Graph, DbViewError> {
    authorize_dbview(principal, ws, "store.graph")?;
    Ok(store_graph(store, ws, table, id, EDGE_TABLES, depth).await?)
}
