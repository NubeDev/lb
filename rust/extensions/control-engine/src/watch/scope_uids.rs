//! Expand an EMPTY watch scope into the appliance's full component-UID set (slice-6 bugfix).
//!
//! **Why this file exists.** The ce-studio engine only pushes COV frames for *explicitly
//! subscribed* components: a `subscribe` with no `components` streams ZERO value frames
//! (verified on the live engine — empty subscribe = 0 frames, `[100008..100013]` = 36
//! frames/4s). So `control-engine.watch` with no `scope.components` (the UI's default —
//! "watch the whole appliance") armed a pump that carried nothing, and the canvas showed
//! no live values. See `docs/debugging/frontend/ce-canvas-empty-cov-scope-no-live-values.md`.
//!
//! One responsibility: walk the tolerant raw tree (`tools::raw_tree`, which passes the
//! engine's `{ nodes, edges }` through verbatim) and collect every component `uid` into a
//! sorted+deduped list, so the caller can turn "empty scope" into "every UID in the
//! subtree" BEFORE arming. The synthetic root (`uid 0`) is skipped — it is a container,
//! not a COV-bearing component. Order-invariant (sorted) so it feeds `series::args_hash`
//! deterministically.

use serde_json::Value;

/// Recursively collect every component `uid` from a raw tree's `nodes` (each node may
/// carry nested `children`). The synthetic root (`uid == 0`) is excluded. Returns a
/// sorted, deduped list — the canonical scope the series is keyed on.
#[must_use]
pub fn collect(tree: &Value) -> Vec<u32> {
    let mut out = Vec::new();
    if let Some(nodes) = tree.get("nodes").and_then(Value::as_array) {
        for node in nodes {
            walk(node, &mut out);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Push this node's `uid` (unless it is the synthetic root `0`) then recurse into
/// `children`. `children` is a JSON array on the live engine, but a real appliance may
/// also emit an object map or omit it entirely — all three are tolerated.
fn walk(node: &Value, out: &mut Vec<u32>) {
    if let Some(uid) = node.get("uid").and_then(Value::as_u64) {
        if uid != 0 {
            out.push(uid as u32);
        }
    }
    match node.get("children") {
        Some(Value::Array(children)) => {
            for child in children {
                walk(child, out);
            }
        }
        Some(Value::Object(children)) => {
            for child in children.values() {
                walk(child, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Rule-9: the fixture is the REAL live-engine tree shape (captured from
    // `GET /api/v0/nodes?depth=-1` on `127.0.0.1:7979`): a synthetic root `uid 0` with a
    // nested `children` array, a `Services` subtree, and the six top-level components the
    // handover proved stream frames (`[100008..100013]`).
    fn live_tree() -> Value {
        json!({
            "nodes": [{
                "uid": 0, "name": "root", "children": [
                    { "uid": 100000, "name": "Services", "children": [
                        { "uid": 100001, "name": "ZenohService", "children": null },
                        { "uid": 100002, "name": "bacnetService", "children": null }
                    ]},
                    { "uid": 100008, "name": "changeOfState", "children": null },
                    { "uid": 100009, "name": "limitAlarm", "children": null },
                    { "uid": 100010, "name": "random", "children": null },
                    { "uid": 100011, "name": "dewpoint", "children": null },
                    { "uid": 100012, "name": "cron", "children": null },
                    { "uid": 100013, "name": "bacnetDevice", "children": null }
                ]
            }],
            "edges": []
        })
    }

    #[test]
    fn collects_every_nested_uid_and_skips_root() {
        let uids = collect(&live_tree());
        // The six verified frame-bearing children are all present...
        for want in [100008, 100009, 100010, 100011, 100012, 100013] {
            assert!(uids.contains(&want), "missing {want} in {uids:?}");
        }
        // ...plus the nested Services subtree...
        assert!(uids.contains(&100000) && uids.contains(&100001) && uids.contains(&100002));
        // ...and the synthetic root is NOT subscribed (it bears no COV).
        assert!(!uids.contains(&0), "root uid 0 must be excluded");
        // Sorted + deduped (order-invariant → deterministic series hash).
        let mut sorted = uids.clone();
        sorted.sort_unstable();
        assert_eq!(uids, sorted);
    }

    #[test]
    fn empty_or_shapeless_tree_yields_no_uids() {
        assert!(collect(&json!({ "nodes": [] })).is_empty());
        assert!(collect(&json!({})).is_empty());
        assert!(collect(&json!({ "nodes": [{ "name": "no-uid" }] })).is_empty());
    }

    #[test]
    fn tolerates_object_children_map() {
        // A real appliance may serialize children as an object map, not an array.
        let tree = json!({ "nodes": [{ "uid": 0, "children": {
            "a": { "uid": 5, "children": null },
            "b": { "uid": 6, "children": null }
        }}]});
        assert_eq!(collect(&tree), vec![5, 6]);
    }
}
