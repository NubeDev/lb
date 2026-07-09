//! The **link-pair** resolver + validator (flow-input-ports-scope "Intent" step 5) — the pure graph
//! math that turns Node-RED's wireless `link-out {target}` / `link-in {name}` pair into the ordinary
//! port-targeted edges the run engine schedules. Pure model math (no I/O, no registry) so it is
//! exercised in unit tests the same way the engine exercises it at run load.
//!
//! **The wireless promise is editor sugar only.** [`resolve_links`] produces a NEW [`Flow`] in which:
//! - each `link-out {target:T}`'s upstream(s) are appended onto the matching `link-in {name:T}`'s
//!   `needs` (the virtual edge made physical, landing on `link-in`'s primary input port);
//! - every `link-out` node is **dropped** from the run graph (its only job was to name a target; the
//!   `link-in` now carries those wires, and the `any`-funnel runtime + the `fctx` seam propagate the
//!   multiplicity downstream — flow-input-ports-scope Slice 2).
//!
//! Resolution is a **run-load** step (the coordinator calls it once at the top of `start`/`drive`),
//! NOT a save-time mutation: the persisted `flow` record keeps the author's `link-out`/`link-in`
//! intact so the editor round-trips the wireless sugar and a deleted `link-out` can never leave a
//! stale resolved wire behind. Save-time only [`validate_links`]s the topology (a `link-out` naming a
//! missing `link-in` is a clear mistake, caught before any run).
//!
//! Design note — why run-load resolution over the scope's literal "save-time" wording: a save-time
//! rewrite that mutates the persisted `flow` is non-idempotent under re-save (the editor loads a flow
//! already carrying resolved wires, then resolves again) and leaves stale resolved wires when a
//! `link-out` is deleted without also deleting the phantom wire the resolver added. Resolving a
//! transient copy at run load sidesteps both (the persisted record is the author's intent; the engine
//! never sees a `link-out`). The rejected alternative is recorded here per the docs convention.

use std::collections::HashMap;

use crate::model::{DagError, Flow};

/// The built-in node type ids the pair wears (core built-ins — branching on them is not a rule-10
/// extension-id leak; the core owns these descriptors).
const LINK_OUT: &str = "link-out";
const LINK_IN: &str = "link-in";

/// The `config.name` of a `link-in` node (its wireless address); `""` when the node is not a
/// `link-in` or carries no name.
fn link_in_name(flow: &Flow, node_id: &str) -> String {
    flow.node(node_id)
        .filter(|n| n.node_type == LINK_IN)
        .and_then(|n| n.config.get("name"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// The `config.target` of a `link-out` node (the `link-in` name it forwards to); `""` when the node
/// is not a `link-out` or carries no target.
fn link_out_target(flow: &Flow, node_id: &str) -> String {
    flow.node(node_id)
        .filter(|n| n.node_type == LINK_OUT)
        .and_then(|n| n.config.get("target"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Validate the link-pair topology (called from `flows.save` after the pure DAG check, before any
/// run). Catches the clear authoring mistakes:
/// - a `link-out {target:T}` whose `T` matches no `link-in {name:T}` — a wireless sender pointing
///   nowhere (rejected, not silently dropped at run);
/// - a node that wires from a `link-out` (lists one in its `needs`) — `link-out` is a naming node
///   whose only output is the wireless name, not a data port; a downstream of it is a mistake;
/// - a `link-in` with **no sources at all** — no `link-out` targets it AND it has no physical wires —
///   a dead node that would never fire (almost certainly a naming typo).
///
/// Same-workspace wall (rule 6) is structural here: link names resolve only within one flow (one ws),
/// so a ws-B `link-out` can never name a ws-A `link-in` — no special handling needed.
pub fn validate_links(flow: &Flow) -> Result<(), DagError> {
    // Index link-in names → their node ids (a name may be claimed by at most one link-in; duplicates
    // are caught as a mistake too — two link-ins answering the same name is ambiguous).
    let mut link_ins_by_name: HashMap<String, Vec<String>> = HashMap::new();
    for n in &flow.nodes {
        if n.node_type == LINK_IN {
            let name = link_in_name(flow, &n.id);
            link_ins_by_name.entry(name).or_default().push(n.id.clone());
        }
    }
    // Reject two link-ins naming the same address — the resolver would funnel both, which silently
    // duplicates firings (ambiguous authoring, not a deliberate fan-out).
    for (name, ids) in &link_ins_by_name {
        if name.is_empty() || ids.len() <= 1 {
            continue;
        }
        return Err(DagError::LinkNameCollision(name.clone(), ids.clone()));
    }

    // Each link-out's target must match ≥1 link-in.
    for n in &flow.nodes {
        if n.node_type != LINK_OUT {
            continue;
        }
        let target = link_out_target(flow, &n.id);
        if target.is_empty() {
            return Err(DagError::LinkOutNoTarget(n.id.clone()));
        }
        if !link_ins_by_name.contains_key(&target) {
            return Err(DagError::LinkOutMissingTarget(n.id.clone(), target));
        }
    }

    // Nothing may wire FROM a link-out (its output is the wireless name, not a data port). A node
    // listing a link-out in its needs is a mistake — that wire vanishes when the link-out is dropped
    // at run load, so it is caught here, not silently.
    let link_out_ids: std::collections::HashSet<&str> = flow
        .nodes
        .iter()
        .filter(|n| n.node_type == LINK_OUT)
        .map(|n| n.id.as_str())
        .collect();
    for n in &flow.nodes {
        for need in &n.needs {
            if link_out_ids.contains(need.as_str()) {
                return Err(DagError::WiresFromLinkOut(n.id.clone(), need.clone()));
            }
        }
    }

    // A link-in with no link-outs targeting it AND no physical wires is a dead node (never fires).
    let mut link_ins_with_senders: std::collections::HashSet<&str> =
        std::collections::HashSet::new();
    for n in &flow.nodes {
        if n.node_type == LINK_OUT {
            let target = link_out_target(flow, &n.id);
            if let Some(ids) = link_ins_by_name.get(&target) {
                for id in ids {
                    link_ins_with_senders.insert(id.as_str());
                }
            }
        }
    }
    for n in &flow.nodes {
        if n.node_type != LINK_IN {
            continue;
        }
        let has_sender = link_ins_with_senders.contains(n.id.as_str());
        let has_physical = !n.needs.is_empty();
        if !has_sender && !has_physical {
            let name = link_in_name(flow, &n.id);
            return Err(DagError::LinkInDead(n.id.clone(), name));
        }
    }
    Ok(())
}

/// Resolve the link pair into ordinary port-targeted edges (run-load; the coordinator calls this once
/// at the top of `start`/`drive` and threads the result through the engine). Returns a NEW [`Flow`]:
/// - each `link-out {target:T}`'s `needs` are appended onto the matching `link-in {name:T}`'s
///   `needs` (idempotent — an upstream already wired onto the `link-in` is not re-added), landing on
///   the `link-in`'s primary input port (the `any` funnel);
/// - every `link-out` node is removed from `flow.nodes` (its job is done; the `link-in` carries the
///   wires and the `fctx` seam propagates the multiplicity).
///
/// Callers must have already passed [`validate_links`] (a missing target is a save-time reject); this
/// function is total over a validated flow and never panics. The persisted `flow` is untouched — the
/// engine runs on the resolved copy, the editor renders the author's wireless sugar verbatim.
pub fn resolve_links(flow: &Flow) -> Flow {
    let mut resolved = flow.clone();

    // Index link-in names → their node ids (validated: each non-empty name has exactly one link-in).
    let mut link_in_by_name: HashMap<String, String> = HashMap::new();
    for n in &resolved.nodes {
        if n.node_type == LINK_IN {
            let name = link_in_name(flow, &n.id);
            if !name.is_empty() {
                link_in_by_name.entry(name).or_insert_with(|| n.id.clone());
            }
        }
    }

    // Collect the virtual edges to add: link_in_id → ordered, de-duplicated upstream ids (preserve
    // the author's ordering across link-outs; drop duplicates so a diamond does not double-fire).
    let mut add_to: HashMap<String, Vec<String>> = HashMap::new();
    let mut link_out_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    for n in &flow.nodes {
        if n.node_type != LINK_OUT {
            continue;
        }
        link_out_ids.insert(n.id.clone());
        let target = link_out_target(flow, &n.id);
        let Some(link_in_id) = link_in_by_name.get(&target) else {
            continue; // (validate_links rejects this; skip defensively at run load)
        };
        let bucket = add_to.entry(link_in_id.clone()).or_default();
        for up in &n.needs {
            if !bucket.contains(up) {
                bucket.push(up.clone());
            }
        }
    }

    // Apply the virtual edges onto each link-in's primary input port. The `to_port` is the primary
    // (None ⇒ the descriptor's first input), so no `InputEdge` is needed — the funnel resolves its
    // single triggering upstream per firing (the `any`-funnel auto-wire from flow-input-ports-scope).
    for n in resolved.nodes.iter_mut() {
        if let Some(ups) = add_to.get(&n.id) {
            for up in ups {
                if !n.needs.contains(up) {
                    n.needs.push(up.clone());
                }
            }
        }
    }

    // Drop the link-out nodes — they are fully resolved (their upstreams now feed the link-in) and
    // nothing wires from them (validate_links guaranteed it). The link-in carries the funnel.
    resolved.nodes.retain(|n| !link_out_ids.contains(&n.id));
    resolved
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Node;
    use serde_json::json;

    fn rhai(id: &str, needs: &[&str]) -> Node {
        Node {
            id: id.into(),
            node_type: "rhai".into(),
            needs: needs.iter().map(|s| s.to_string()).collect(),
            inputs: Vec::new(),
            with: serde_json::Map::new(),
            config: json!({ "source": "payload" }),
            position: None,
        }
    }

    fn link_out(id: &str, target: &str, needs: &[&str]) -> Node {
        Node {
            id: id.into(),
            node_type: LINK_OUT.into(),
            needs: needs.iter().map(|s| s.to_string()).collect(),
            inputs: Vec::new(),
            with: serde_json::Map::new(),
            config: json!({ "target": target }),
            position: None,
        }
    }

    fn link_in(id: &str, name: &str, needs: &[&str]) -> Node {
        Node {
            id: id.into(),
            node_type: LINK_IN.into(),
            needs: needs.iter().map(|s| s.to_string()).collect(),
            inputs: Vec::new(),
            with: serde_json::Map::new(),
            config: json!({ "name": name }),
            position: None,
        }
    }

    fn flow(nodes: Vec<Node>) -> Flow {
        Flow {
            workspace: "ws".into(),
            id: "f".into(),
            name: "f".into(),
            version: 1,
            params: serde_json::Map::new(),
            nodes,
            failure_policy: crate::model::FailurePolicy::Halt,
            deleted: false,
            enabled: true,
            start_on_boot: false,
            placement: crate::model::Placement::Either,
            concurrency: Default::default(),
            cron: None,
            next_attempt_ts: 0,
        }
    }

    #[test]
    fn resolve_rewrites_link_out_upstreams_onto_link_in_and_drops_the_senders() {
        // a, b, c → link-out{T} → link-in{T} → W. After resolution link-out nodes are gone and
        // link-in carries [a, b, c] on its primary (any) port.
        let f = flow(vec![
            rhai("a", &[]),
            rhai("b", &[]),
            rhai("c", &[]),
            link_out("lo-a", "T", &["a"]),
            link_out("lo-b", "T", &["b"]),
            link_out("lo-c", "T", &["c"]),
            link_in("li", "T", &[]),
            rhai("w", &["li"]),
        ]);
        validate_links(&f).expect("valid link topology");
        let r = resolve_links(&f);
        // link-out nodes are dropped.
        assert!(r.node("lo-a").is_none());
        assert!(r.node("lo-b").is_none());
        assert!(r.node("lo-c").is_none());
        // link-in now needs a, b, c (the resolved virtual edges), in author order, de-duplicated.
        let li = r.node("li").unwrap();
        assert_eq!(li.needs, vec!["a", "b", "c"]);
        // W still needs link-in (the funnel's downstream — where the fctx propagates).
        assert_eq!(r.node("w").unwrap().needs, vec!["li"]);
    }

    #[test]
    fn resolve_is_idempotent_on_a_link_in_that_also_has_physical_wires() {
        // link-in with a physical wire from `phys` AND a link-out from `a` ⇒ both land on its needs.
        let f = flow(vec![
            rhai("phys", &[]),
            rhai("a", &[]),
            link_out("lo", "T", &["a"]),
            link_in("li", "T", &["phys"]),
            rhai("w", &["li"]),
        ]);
        validate_links(&f).expect("physical + virtual is fine");
        let r = resolve_links(&f);
        let li = r.node("li").unwrap();
        assert_eq!(li.needs, vec!["phys", "a"]);
    }

    #[test]
    fn resolve_dedupes_a_diamond_upstream_shared_by_two_link_outs() {
        // Two link-outs both forwarding the SAME upstream `shared` ⇒ link-in sees it once (no double-
        // fire; the any-funnel settles per distinct upstream, not per link-out).
        let f = flow(vec![
            rhai("shared", &[]),
            link_out("lo1", "T", &["shared"]),
            link_out("lo2", "T", &["shared"]),
            link_in("li", "T", &[]),
        ]);
        validate_links(&f).expect("two senders one upstream is valid");
        let r = resolve_links(&f);
        assert_eq!(r.node("li").unwrap().needs, vec!["shared"]);
    }

    #[test]
    fn validate_rejects_a_link_out_naming_a_missing_link_in() {
        let f = flow(vec![rhai("a", &[]), link_out("lo", "nope", &["a"])]);
        assert_eq!(
            validate_links(&f),
            Err(DagError::LinkOutMissingTarget("lo".into(), "nope".into()))
        );
    }

    #[test]
    fn validate_rejects_a_link_out_with_no_target() {
        let mut lo = link_out("lo", "T", &["a"]);
        lo.config = json!({});
        let f = flow(vec![rhai("a", &[]), lo]);
        assert_eq!(
            validate_links(&f),
            Err(DagError::LinkOutNoTarget("lo".into()))
        );
    }

    #[test]
    fn validate_rejects_a_wire_from_a_link_out() {
        // A node must not wire from a link-out (its output is the wireless name, not a data port).
        let f = flow(vec![
            rhai("a", &[]),
            link_out("lo", "T", &["a"]),
            link_in("li", "T", &[]),
            rhai("bad", &["lo"]),
        ]);
        assert_eq!(
            validate_links(&f),
            Err(DagError::WiresFromLinkOut("bad".into(), "lo".into()))
        );
    }

    #[test]
    fn validate_rejects_a_link_in_with_no_sources_at_all() {
        // No link-out targets it AND no physical wire ⇒ dead node (naming typo).
        let f = flow(vec![link_in("li", "lonely", &[])]);
        assert_eq!(
            validate_links(&f),
            Err(DagError::LinkInDead("li".into(), "lonely".into()))
        );
    }

    #[test]
    fn validate_accepts_a_link_in_with_only_physical_wires() {
        // A link-in with a physical wire but no link-outs is live (no dead-node reject).
        let f = flow(vec![rhai("a", &[]), link_in("li", "lonely", &["a"])]);
        validate_links(&f).expect("physical wire keeps it live");
    }

    #[test]
    fn validate_rejects_two_link_ins_sharing_one_name() {
        let f = flow(vec![
            rhai("a", &[]),
            link_out("lo", "T", &["a"]),
            link_in("li1", "T", &[]),
            link_in("li2", "T", &[]),
        ]);
        assert_eq!(
            validate_links(&f),
            Err(DagError::LinkNameCollision(
                "T".into(),
                vec!["li1".into(), "li2".into()]
            ))
        );
    }
}
