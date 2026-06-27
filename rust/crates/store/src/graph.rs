//! Read a depth- and fan-out-bounded slice of the workspace graph — record **nodes** and the graph
//! **edges** that already exist as relation records — for the admin DB-browser's react-flow view
//! (data-console scope). A non-SQL user *follows* a relationship by clicking, instead of writing a
//! traversal nobody can spell.
//!
//! First cut, deliberately small (scope: "keep the first cut depth-1 and click-to-expand"): the only
//! edges drawn are **real relation records** — a SurrealDB `RELATE in -> edge -> out`. We never
//! synthesise edges from a join. The edge tables to walk are a **parameter** (`edge_tables`) so this
//! crate stays generic: the host passes the relation tables it knows (today: `tagged`), and `lb_store`
//! takes no dependency on the tag layer. A seed is either a whole table (its rows are the seed nodes)
//! or a single record id (expand-on-click).
//!
//! Namespace-bound (`use_ws`): a ws-A graph is physically A's records only (the hard wall, §7). Both
//! the seed fan-out and the per-node edge fan-out are **hard-capped** ([`MAX_SEED`]/[`MAX_FANOUT`]) so
//! a 1M-row table or a hub node with thousands of edges can never return the whole tenant. The
//! capability gate (admin-only) is one layer up in the host `dbview` service.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::open::{Store, StoreError};

/// Hard cap on seed nodes pulled from a table (the first cut is depth-1, click-to-expand — we never
/// auto-lay-out a whole tenant).
pub const MAX_SEED: usize = 50;
/// Hard cap on edges read per node per edge-table, so a hub node with thousands of relations can't
/// blow up the view.
pub const MAX_FANOUT: usize = 50;

/// A react-flow node: the record id (`table:id`, used directly as the stable node id — it is already
/// unique) and a `kind` (the table) the UI styles/labels by.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Node {
    pub id: String,
    pub kind: String,
}

/// A react-flow edge between two node ids, labelled by the relation table it came from.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub label: String,
}

/// The graph slice: deduped nodes + edges, ready for react-flow.
#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

/// Build a bounded graph slice in `ws`. The seed is `table` (its first [`MAX_SEED`] rows become seed
/// nodes) and/or a single `id` (`table:id`) to expand. `edge_tables` are the relation tables to walk
/// (the host supplies them — generic here). `_depth` is accepted for the contract but the first cut is
/// fixed at depth-1 (seed → its neighbours); deeper traversal is click-to-expand from the UI (call
/// again with a neighbour id).
pub async fn graph(
    store: &Store,
    ws: &str,
    table: Option<&str>,
    id: Option<&str>,
    edge_tables: &[&str],
    _depth: u32,
) -> Result<Graph, StoreError> {
    let mut nodes: BTreeMap<String, Node> = BTreeMap::new();
    let mut edges: Vec<Edge> = Vec::new();

    // 1. Seed nodes — either the first MAX_SEED rows of a table, or a single record.
    let seeds: Vec<String> = match (table, id) {
        (_, Some(rid)) => vec![rid.to_string()],
        (Some(tb), None) => {
            let mut resp = store
                .query_ws(
                    ws,
                    &format!(
                        "SELECT meta::id(id) AS rid, id AS _oid FROM type::table($tb) \
                         ORDER BY _oid ASC LIMIT {MAX_SEED}"
                    ),
                    vec![("tb".into(), serde_json::Value::String(tb.to_string()))],
                )
                .await?;
            let raw: Vec<SeedRow> = resp
                .take(0)
                .map_err(|e| StoreError::Decode(e.to_string()))?;
            raw.into_iter()
                .map(|r| format!("{tb}:{}", render_id(&r.rid)))
                .collect()
        }
        (None, None) => {
            return Ok(Graph {
                nodes: vec![],
                edges: vec![],
            })
        }
    };

    for seed in &seeds {
        nodes.entry(seed.clone()).or_insert_with(|| Node {
            id: seed.clone(),
            kind: table_of(seed),
        });

        // 2. Walk each relation table for the seed's outgoing edges. A relation edge carries the
        //    source as `in` (a Thing) — but its string form is backtick-escaped (`` series:`x` ``),
        //    which won't equal a clean `table:id` seed, and a seed may be a composite record id
        //    (`series` is keyed on `[series, producer, seq]`) a `type::thing` reconstruction can't
        //    rebuild. So we match on the edge's **denormalized entity string** `ent` — the exact
        //    `table:id` reference the relation was created against — which round-trips cleanly. `out`
        //    is read as table + id strings so a heterogeneous neighbour set deserializes. Bounded by
        //    MAX_FANOUT. (`ent` is the lb-tags edge convention; the host supplies these edge tables.)
        for et in edge_tables {
            let mut resp = store
                .query_ws(
                    ws,
                    &format!(
                        "SELECT meta::tb(out) AS otb, meta::id(out) AS oid FROM type::table($et) \
                         WHERE ent = $seed LIMIT {MAX_FANOUT}"
                    ),
                    vec![
                        ("et".into(), serde_json::Value::String((*et).to_string())),
                        ("seed".into(), serde_json::Value::String(seed.clone())),
                    ],
                )
                .await?;
            let rels: Vec<RelRow> = resp
                .take(0)
                .map_err(|e| StoreError::Decode(e.to_string()))?;

            for r in rels {
                let target = format!("{}:{}", r.otb, render_id(&r.oid));
                nodes.entry(target.clone()).or_insert_with(|| Node {
                    id: target.clone(),
                    kind: r.otb.clone(),
                });
                edges.push(Edge {
                    source: seed.clone(),
                    target,
                    label: (*et).to_string(),
                });
            }
        }
    }

    Ok(Graph {
        nodes: nodes.into_values().collect(),
        edges,
    })
}

/// Render a record id (string, number, or composite array — a `series` is keyed on a 3-tuple) into
/// its displayable id half. A bare string verbatim; structured ids as compact JSON.
fn render_id(rid: &serde_json::Value) -> String {
    match rid {
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// The table half of a `table:id` reference (everything before the first `:`).
fn table_of(record: &str) -> String {
    record
        .split_once(':')
        .map(|(t, _)| t.to_string())
        .unwrap_or_else(|| record.to_string())
}

#[derive(Deserialize)]
struct SeedRow {
    rid: serde_json::Value,
}

#[derive(Deserialize)]
struct RelRow {
    otb: String,
    oid: serde_json::Value,
}
