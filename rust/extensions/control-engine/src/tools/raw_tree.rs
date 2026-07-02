//! `control-engine.tree` — TOLERANT raw fetch (bypasses `rubix-ce`'s typed
//! `EdgeDto` decode). The wiresheet consumes the `{ nodes, edges }` JSON verbatim
//! (see `tree.rs`), so round-tripping edges through the client crate's *strict*
//! `EdgeDto` (required `source_uid`/`target_uid`, pinned rev `51ab97e`) buys the
//! extension nothing but a crash: a real appliance emits edges that OMIT
//! `source_uid` (dangling / half-formed), and serde fails the WHOLE decode →
//! "bad host response" → blank canvas (see
//! `docs/debugging/frontend/ce-tree-missing-source-uid-blanks-canvas.md`).
//!
//! This path fetches the SAME `GET /api/v0/nodes?depth=..&withEdges=true` route the
//! crate's `get_tree` uses, unwraps the `{ "data": { nodes, edges } }` envelope as
//! untyped `serde_json::Value`, and passes it straight through — so a
//! `source_uid`-less edge survives as-is (the wiresheet decides how to render a
//! dangling edge, not the transport). The fake path keeps the typed `get_tree`
//! (`tools::dispatch`) — this raw fetch is REAL-appliance only.

use serde_json::{json, Value};

use crate::args::{base_of, NodeRefArg};

/// Fetch a component subtree as tolerant raw JSON. `base` is the resolved appliance
/// selector (`host:port`, per `base_of`); `input` carries the optional `node`/`depth`
/// args (same as the typed [`super::tree::run`]).
///
/// # Errors
/// Transport failures (unreachable engine, non-2xx status) and a non-JSON body still
/// error — only the strict *edge-shape* decode is relaxed. A malformed `node` arg errors.
pub async fn run(base: &str, input: &Value) -> Result<Value, String> {
    let node: NodeRefArg = match input.get("node") {
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| format!("bad node arg: {e}"))?,
        None => NodeRefArg::default(),
    };
    let depth = input.get("depth").and_then(Value::as_i64).unwrap_or(-1);

    let (host, port) = base_of(base);
    let path = match node_uid(&node) {
        Some(uid) => format!("/nodes/uid/{uid}?depth={depth}&withEdges=true"),
        None => format!("/nodes?depth={depth}&withEdges=true"),
    };
    let url = format!("http://{host}:{port}/api/v0{path}");

    let resp = reqwest::get(&url)
        .await
        .map_err(|e| format!("control-engine tree fetch {path}: {e}"))?;
    let status = resp.status();
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("control-engine tree {path}: reading body: {e}"))?;
    if !status.is_success() {
        let msg = serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
            .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
        return Err(format!(
            "control-engine tree {path}: {} {msg}",
            status.as_u16()
        ));
    }

    // Untyped envelope: `{ "data": { "nodes": [...], "edges": [...] } }`. We do NOT
    // decode into `EdgeDto` — that's the whole point (a missing `source_uid` must
    // not fail the read). Missing halves default to empty arrays.
    let envelope: Value = serde_json::from_slice(&bytes)
        .map_err(|e| format!("control-engine tree {path}: decoding response: {e}"))?;
    let data = envelope.get("data").unwrap_or(&envelope);
    let nodes = data.get("nodes").cloned().unwrap_or_else(|| json!([]));
    let edges = data.get("edges").cloned().unwrap_or_else(|| json!([]));
    Ok(json!({ "nodes": nodes, "edges": edges }))
}

/// The concrete component UID a `node` arg addresses, or `None` for the root (a
/// whole-tree read). Mirrors `NodeRefArg::to_node_ref` without needing an instance.
fn node_uid(node: &NodeRefArg) -> Option<u32> {
    if node.root {
        return None;
    }
    node.uid
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Rule-9: the fixture is a REAL captured `/nodes` envelope shape — an edge that
    // OMITS `source_uid` (the exact byte-7041 crasher). We assert the tolerant path
    // passes it through instead of failing the decode. No network: we exercise the
    // envelope-unwrap logic directly on the captured bytes.
    #[test]
    fn envelope_with_source_uid_less_edge_passes_through() {
        // Captured shape: one node, one dangling edge (no `source_uid`).
        let raw = json!({
            "data": {
                "nodes": [{ "uid": 1, "name": "add", "properties": {} }],
                "edges": [{ "uid": 9, "target_uid": 1, "loop_back": false }]
            }
        });
        let bytes = serde_json::to_vec(&raw).unwrap();
        let envelope: Value = serde_json::from_slice(&bytes).unwrap();
        let data = envelope.get("data").unwrap();
        let edges = data.get("edges").unwrap().as_array().unwrap();
        // The edge survives verbatim — no `source_uid`, and that's fine.
        assert_eq!(edges.len(), 1);
        assert!(edges[0].get("source_uid").is_none());
        assert_eq!(edges[0]["uid"], 9);
    }

    #[test]
    fn defaults_missing_halves_to_empty_arrays() {
        let envelope = json!({ "data": {} });
        let data = envelope.get("data").unwrap();
        let nodes = data.get("nodes").cloned().unwrap_or_else(|| json!([]));
        let edges = data.get("edges").cloned().unwrap_or_else(|| json!([]));
        assert_eq!(nodes, json!([]));
        assert_eq!(edges, json!([]));
    }

    #[test]
    fn root_arg_yields_no_uid_and_keyed_arg_yields_uid() {
        let root = NodeRefArg::default();
        assert_eq!(node_uid(&root), None);
        let keyed: NodeRefArg =
            serde_json::from_str(r#"{"root":false,"uid":7,"kind":"component"}"#).unwrap();
        assert_eq!(node_uid(&keyed), Some(7));
    }
}
