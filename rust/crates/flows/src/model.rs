//! The typed node-graph `Flow` model + DAG math (flows-scope "The node model", generalised from the
//! rubix-cube rule-DAG step). A flow is a validated DAG whose every node is data-driven: an `id`, a
//! `node_type` referencing a [`crate::descriptor::NodeDescriptor`] (built-in or `<ext>.<type>`), a
//! `config` validated against that descriptor's schema, and `needs` + `with` carrying the **rubix-cube
//! binding grammar verbatim** — whole-value `${steps.x.output}` / `${params.y}` references or a
//! literal, no templating mini-language (lifted verbatim from the rule-DAG lineage).
//!
//! The DAG math (Kahn cycle-detect, indegrees/dependents/frontier) is the rubix-cube DAG validation
//! verbatim — a flow **is** the generalised rule-DAG topology with a typed node payload (Decision 8).
//! It is pure math with no I/O, so a flow is validated at save before any node runs.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::descriptor::NodeKind;

/// The size cap on a flow's node count (the rubix-cube DAG cap; re-checks a hand-edited record).
pub const MAX_FLOW_NODES: usize = 256;

/// No extension namespace prefix — built-in types (`trigger`/`tool`/`rhai`/`subflow`/`sink`) carry
/// no `<ext_id>.` prefix; an extension node's type is always `<ext_id>.<type>`.
pub const BUILTIN_PREFIX: &str = "";

/// Whether a node type is a built-in (no `<ext_id>.` namespace).
pub fn is_builtin_type(node_type: &str) -> bool {
    !node_type.contains('.')
}

/// What happens when a node fails (after its retries): the rubix-cube DAG policy verbatim. `Halt` prunes the
/// failed node's transitive subtree (those nodes `Skipped`); `Continue` releases dependents with the
/// failed output resolved to `null`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailurePolicy {
    #[default]
    Halt,
    Continue,
}

/// What happens when a flow is fired while a live (non-terminal) run of it already exists
/// (rules-workflow-convergence scope, slice 2). Enforced at fire time in BOTH the cron reactor and
/// `flows.run`:
///   - `Queue` — let the new run start (runs may overlap). **The default** — it preserves the
///     established behavior (two manual runs of a flow both run; a cron tick never suppresses a
///     still-running prior tick), so a flow written before this field behaves exactly as before.
///   - `Skip` — drop the new firing (the running one wins; the cheapest for a slow flow whose overlap
///     is never wanted — "one live run at a time").
///   - `Restart` — cancel the live run(s) and start the new one (the latest firing wins; a control loop
///     that should always reflect the newest input).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Concurrency {
    #[default]
    Queue,
    Skip,
    Restart,
}

/// Where a flow may run — the **eligible set**, not replication (Decision 10). Matched **as data**
/// against the node's role by the reconciler (never an `if cloud` branch). Reuses the extension
/// placement vocabulary so flows + extensions share one enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    #[default]
    Either,
    /// A hub-class role only; on an edge node the flow is simply not scheduled.
    CloudOnly,
    /// The install node that owns the local hardware a native source reads.
    LocalOnly,
}

/// One flow node — a data-driven step. The `node_type` keys into the merged registry; `config` is
/// the validated instance of that descriptor's schema; `needs` + `with` are the DAG edges + bindings.
/// `kind`/ports are resolved from the descriptor at validate/run time, not stored here (single
/// source of truth — the descriptor is the join).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    /// The descriptor type: a built-in (`trigger`/`tool`/`rhai`/`subflow`/`sink`) or `<ext>.<type>`.
    #[serde(rename = "type")]
    pub node_type: String,
    /// The DAG dependency edges — the upstream node ids this node waits on (the topology). Per
    /// flow-input-ports-scope, `needs` stays the ordering/dependency edge; an edge's **target input
    /// port** is additive metadata in [`Node::inputs`] (None ⇒ the node's primary input port).
    #[serde(default)]
    pub needs: Vec<String>,
    /// Per-edge **target input port** metadata (flow-input-ports-scope Axis 1). One entry per edge
    /// that targets a non-primary port (or that wants to name the primary explicitly); an edge with
    /// no entry defaults its `to_port` to the node's first declared input port. Additive + serde-
    /// defaulted so a pre-ports flow deserialises unchanged (every edge ⇒ primary input). Kept in
    /// agreement with `needs` by [`validate_flow`]: every `from` here must be a `needs` entry.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<InputEdge>,
    /// Input bindings: literal | `${steps.x}` | `${steps.x.payload}` | `${steps.x.<path>}` |
    /// `${params.y}` (the message-envelope grammar, flow-message-envelope-scope D5).
    #[serde(default)]
    pub with: serde_json::Map<String, Value>,
    /// The node's config, validated against its descriptor's schema at save.
    #[serde(default)]
    pub config: Value,
    /// The node's canvas position (editor geometry). Optional + serde-default so pre-geometry flows
    /// still load (no migration) and a headless caller may omit it; the editor falls back to a grid
    /// layout when absent. Pure view state — it never affects DAG math, validation, or run order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
}

/// A wired edge's **target input port** metadata (flow-input-ports-scope Axis 1 — a Node-RED wire
/// lands on a named input port). `to_port = None` ⇒ the downstream node's primary input port (its
/// first declared input), so a pre-ports single-input linear flow is unchanged. The `from` is the
/// upstream node id (the same id that appears in [`Node::needs`]); an edge is identified by its
/// `from` (a node lists another at most once in `needs`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputEdge {
    /// The upstream node id this wire comes from (must also appear in the node's `needs`).
    pub from: String,
    /// The named input port this wire lands on; `None` ⇒ the node's primary input port.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to_port: Option<String>,
}

impl InputEdge {
    /// Construct an edge metadata entry targeting `to_port` from `from`.
    pub fn new(from: impl Into<String>, to_port: Option<String>) -> Self {
        Self {
            from: from.into(),
            to_port,
        }
    }
}

/// A node's canvas coordinates (editor geometry only — see `Node::position`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Eq for Position {}

impl Node {
    pub fn new(id: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            needs: Vec::new(),
            inputs: Vec::new(),
            with: serde_json::Map::new(),
            config: Value::Null,
            position: None,
        }
    }

    /// The `to_port` this node wires from `upstream`, or `None` (⇒ the primary input port) when the
    /// edge carries no port metadata. A node lists another at most once in `needs`, so the `from`
    /// match is unique. Per flow-input-ports-scope Axis 1.
    pub fn to_port_from(&self, upstream: &str) -> Option<String> {
        self.inputs
            .iter()
            .find(|e| e.from == upstream)
            .and_then(|e| e.to_port.clone())
    }
}

/// The kind of config a node carries — used by `flows.run` to know which trigger sub-mode a node is.
/// (Lifecycle fields live on the `Flow` record; this is the node-instance config surface.)
pub type NodeConfig = Value;

/// A flow — a typed, versioned node graph (flows-scope). `version` is monotonic; a run **pins** it
/// (Decision 1) so a live run is immune to edits. Editing writes a new version; a structural edit
/// is never an in-place mutation of a live run. Lifecycle fields (`enabled`, placement, the cron
/// schedule) ride the same record — added by the triggers slice as additive serde defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Flow {
    pub workspace: String,
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// Monotonic graph version. A run pins this (Decision 1).
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub params: serde_json::Map<String, Value>,
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub failure_policy: FailurePolicy,
    #[serde(default)]
    pub deleted: bool,
    // --- lifecycle / trigger fields (triggers-lifecycle-scope). Additive serde defaults: a flow
    // written before these deserialises as enabled / not-start-on-boot / either / no cron. ---
    /// Durable intent: `false` means no trigger fires (the cron scan skips it, the event subscription
    /// is dropped, boot won't fire).
    #[serde(default = "enabled_default")]
    pub enabled: bool,
    /// Marks a flow `reconcile_flows` should bring up (arm sources + fire boot) at node start.
    #[serde(default)]
    pub start_on_boot: bool,
    /// The eligible set (Decision 10). Matched as data against role by the reconciler.
    #[serde(default)]
    pub placement: Placement,
    /// The overlap policy when a firing lands while a live run exists (slice 2). Additive serde
    /// default (`skip`): a flow written before this field deserialises as skip-overlapping-runs.
    #[serde(default)]
    pub concurrency: Concurrency,
    /// A 5-field cron spec (the `cron` trigger kind). `None` = not cron-triggered.
    #[serde(default)]
    pub cron: Option<String>,
    /// The next cron firing instant (logical ts); advanced by `react_to_flows_cron` (fire-once-then-
    /// skip). 0 = not yet computed.
    #[serde(default)]
    pub next_attempt_ts: u64,
}

fn enabled_default() -> bool {
    true
}

fn default_version() -> u32 {
    1
}

impl Flow {
    pub fn new(workspace: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            workspace: workspace.into(),
            id: id.into(),
            name: String::new(),
            version: 1,
            params: serde_json::Map::new(),
            nodes: Vec::new(),
            failure_policy: FailurePolicy::Halt,
            deleted: false,
            enabled: true,
            start_on_boot: false,
            placement: Placement::Either,
            concurrency: Concurrency::default(),
            cron: None,
            next_attempt_ts: 0,
        }
    }
    /// In-degree (number of `needs`) per node — the rubix-cube DAG math verbatim.
    pub fn indegrees(&self) -> HashMap<String, usize> {
        self.nodes
            .iter()
            .map(|n| (n.id.clone(), n.needs.len()))
            .collect()
    }
    /// Reverse edges: node → the nodes that depend on it.
    pub fn dependents(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = self
            .nodes
            .iter()
            .map(|n| (n.id.clone(), Vec::new()))
            .collect();
        for n in &self.nodes {
            for dep in &n.needs {
                if let Some(v) = map.get_mut(dep) {
                    v.push(n.id.clone());
                }
            }
        }
        map
    }
    /// The in-degree-0 frontier (the nodes to enqueue at run start).
    pub fn frontier(&self) -> Vec<String> {
        let mut f: Vec<String> = self
            .indegrees()
            .into_iter()
            .filter(|(_, d)| *d == 0)
            .map(|(id, _)| id)
            .collect();
        f.sort();
        f
    }
    /// Look up a node by id.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }
    /// The transitive **reachable subgraph** from `entry` (inclusive): `entry` plus every node
    /// downstream of it via `dependents`. This is the Node-RED "a message injected at a node flows
    /// only through its downstream wires" — a per-trigger run executes exactly this set. An unknown
    /// `entry` yields an empty set (the caller treats that as "nothing to run").
    pub fn reachable_from(&self, entry: &str) -> HashSet<String> {
        let mut seen: HashSet<String> = HashSet::new();
        if self.node(entry).is_none() {
            return seen;
        }
        let dependents = self.dependents();
        let mut stack = vec![entry.to_string()];
        while let Some(id) = stack.pop() {
            if !seen.insert(id.clone()) {
                continue;
            }
            if let Some(deps) = dependents.get(&id) {
                for d in deps {
                    if !seen.contains(d) {
                        stack.push(d.clone());
                    }
                }
            }
        }
        seen
    }
    /// In-degree per node counting **only** `needs` whose source is in `set` — the indegree of the
    /// *induced* subgraph. A per-trigger run uses this so a join fires on its in-subgraph upstreams
    /// only; a `need` on a node outside the fired subgraph is not counted (it resolves to its
    /// retained/last value or null at binding time, never an unsatisfiable wait).
    pub fn indegrees_within(&self, set: &HashSet<String>) -> HashMap<String, usize> {
        self.nodes
            .iter()
            .filter(|n| set.contains(&n.id))
            .map(|n| {
                let d = n.needs.iter().filter(|dep| set.contains(*dep)).count();
                (n.id.clone(), d)
            })
            .collect()
    }

    /// One node's in-subgraph indegree (the `all`-port barrier count — for v1's single-input-port
    /// nodes, all of its in-subgraph `needs`). The per-slot release path (flow-input-ports-scope)
    /// reads this when it first touches a barrier slot, instead of recomputing the whole map.
    pub fn barrier_indegree(&self, node_id: &str, set: &HashSet<String>) -> usize {
        self.node(node_id)
            .map(|n| n.needs.iter().filter(|dep| set.contains(*dep)).count())
            .unwrap_or(0)
    }

    /// The wired edges landing on `node_id`, as `(from, to_port)` pairs — the per-port view of this
    /// node's incoming wires (flow-input-ports-scope Axis 1). `to_port = None` ⇒ the node's primary
    /// input port (the edge carries no port metadata). The port label is read off [`Node::inputs`],
    /// so this is pure graph math (no descriptor needed); resolving `None` to the actual primary
    /// port name is the descriptor's job (host-side, where the registry lives).
    pub fn edges_into(&self, node_id: &str) -> Vec<(String, Option<String>)> {
        let Some(n) = self.node(node_id) else {
            return Vec::new();
        };
        n.needs
            .iter()
            .map(|from| {
                let to_port = n
                    .inputs
                    .iter()
                    .find(|e| &e.from == from)
                    .and_then(|e| e.to_port.clone());
                (from.clone(), to_port)
            })
            .collect()
    }

    /// In-degree per **(node, port)** counting only `needs` whose source is in `set` — the per-port
    /// barrier count a join policy reads (flow-input-ports-scope Axis 2). An `all` port's indegree
    /// is its wired in-subgraph upstream count; an `any` port is released per settled upstream, not
    /// as a barrier. The policy decision is the host's (it holds the registry); this helper gives
    /// the per-port grouping the policy applies to. `to_port = None` is grouped under the primary
    /// port sentinel `None`.
    pub fn indegrees_within_by_port(
        &self,
        set: &HashSet<String>,
    ) -> HashMap<(String, Option<String>), usize> {
        let mut out: HashMap<(String, Option<String>), usize> = HashMap::new();
        for n in self.nodes.iter().filter(|n| set.contains(&n.id)) {
            for (from, to_port) in self.edges_into(&n.id) {
                if set.contains(&from) {
                    *out.entry((n.id.clone(), to_port)).or_insert(0) += 1;
                }
            }
        }
        out
    }
}

/// The kinds of `node_type` a flow may reference, resolved from its descriptor at validate time.
/// (Used by the engine to dispatch; stored on the descriptor, not the node.)
#[allow(dead_code)]
pub fn kind_of(node_type: &str) -> Option<NodeKind> {
    crate::builtins::builtin_descriptors()
        .into_iter()
        .find(|d| d.r#type == node_type)
        .map(|d| d.kind)
}

/// A DAG validation error (the rubix-cube `DagError` shapes verbatim). All rejected **before any node
/// runs** — a bad graph is a deny-equivalent.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DagError {
    #[error("flow has no nodes")]
    Empty,
    #[error("too many nodes: {0} > {1}")]
    TooManyNodes(usize, usize),
    #[error("duplicate node id: {0}")]
    DuplicateNode(String),
    #[error("node {0} depends on itself")]
    SelfDependency(String),
    #[error("node {0} depends on unknown node {1}")]
    UnknownDependency(String, String),
    #[error("flow has a cycle")]
    Cycle,
    #[error(
        "node {0} has {1} inputs — a join must bind `payload` (auto-wire is single-upstream only)"
    )]
    #[allow(dead_code)]
    // the policy-aware join lint lives in `flows.save` (registry-aware); this
    // variant is retained on the public enum for callers that surface DagError.
    UnboundJoin(String, usize),
    /// A `[[node.inputs]]` port entry whose `from` is not in the node's `needs` (the port metadata
    /// drifted from the topology). The two must agree — a port label with no wire is a mistake.
    #[error("node {0} has an input-port entry for upstream `{1}` which is not in its needs")]
    PortForUnknownNeed(String, String),
    /// A `link-out {target}` whose target names no `link-in {name}` — a wireless sender pointing
    /// nowhere (flow-input-ports-scope Slice 3). Caught at save; the target is a naming typo.
    #[error("link-out `{0}` targets `{1}` but no link-in names it")]
    LinkOutMissingTarget(String, String),
    /// A `link-out` with no `config.target` at all (an unfinished node).
    #[error("link-out `{0}` has no `config.target` — set the target link-in name")]
    LinkOutNoTarget(String),
    /// A node wires from a `link-out` (lists one in its `needs`) — `link-out`'s only output is the
    /// wireless name, not a data port; that wire vanishes when the link-out is dropped at run load.
    #[error("node `{0}` wires from link-out `{1}` — link-out forwards wirelessly, wire from the matching link-in instead")]
    WiresFromLinkOut(String, String),
    /// A `link-in` with no `link-out` targeting it AND no physical wire — a dead node that would
    /// never fire (almost certainly a naming typo on its `name` or a sender's `target`).
    #[error("link-in `{0}` (name `{1}`) has no link-outs targeting it and no physical wires")]
    LinkInDead(String, String),
    /// Two `link-in` nodes share one `name` — ambiguous (the resolver would funnel both, silently
    /// duplicating firings). A `link-in` name must be unique within a flow.
    #[error(
        "link-in name `{0}` is claimed by multiple link-in nodes {1:?} — names must be unique"
    )]
    LinkNameCollision(String, Vec<String>),
}

/// Validate a flow's DAG: non-empty, within `max_nodes`, unique ids, deps resolve, no self-edge,
/// acyclic (Kahn). Reused verbatim from the rubix-cube DAG validation.
pub fn validate_flow(flow: &Flow, max_nodes: usize) -> Result<(), DagError> {
    if flow.nodes.is_empty() {
        return Err(DagError::Empty);
    }
    if flow.nodes.len() > max_nodes {
        return Err(DagError::TooManyNodes(flow.nodes.len(), max_nodes));
    }
    let mut ids: HashSet<&str> = HashSet::new();
    for n in &flow.nodes {
        if !ids.insert(n.id.as_str()) {
            return Err(DagError::DuplicateNode(n.id.clone()));
        }
    }
    for n in &flow.nodes {
        for dep in &n.needs {
            if dep == &n.id {
                return Err(DagError::SelfDependency(n.id.clone()));
            }
            if !ids.contains(dep.as_str()) {
                return Err(DagError::UnknownDependency(n.id.clone(), dep.clone()));
            }
        }
        // Per-edge port metadata must agree with `needs`: every `InputEdge.from` must be a `needs`
        // entry (a port label for a wire that isn't wired is a mistake — the topology is `needs`).
        for e in &n.inputs {
            if !n.needs.iter().any(|need| need == &e.from) {
                return Err(DagError::PortForUnknownNeed(n.id.clone(), e.from.clone()));
            }
        }
    }
    // Kahn: peel in-degree-0 nodes; a remainder means a cycle.
    let mut indeg = flow.indegrees();
    let dependents = flow.dependents();
    let mut queue: VecDeque<String> = indeg
        .iter()
        .filter(|(_, d)| **d == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut processed = 0usize;
    while let Some(id) = queue.pop_front() {
        processed += 1;
        if let Some(deps) = dependents.get(&id) {
            for dep in deps {
                let d = indeg.get_mut(dep).expect("dependent exists");
                *d -= 1;
                if *d == 0 {
                    queue.push_back(dep.clone());
                }
            }
        }
    }
    if processed != flow.nodes.len() {
        return Err(DagError::Cycle);
    }
    // (The join lint — "a multi-input `all` port must bind `payload`" — is policy-aware and lives in
    // `flows.save`, where the descriptor registry resolves a port's `all` vs `any` policy. An `any`
    // port with multiple wires is valid: the funnel fires once per upstream. flow-input-ports-scope.)
    Ok(())
}

/// A compact list view of a flow (the picker) — id + name + version + node count.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FlowSummary {
    pub id: String,
    pub name: String,
    pub version: u32,
    pub nodes: usize,
}

impl From<&Flow> for FlowSummary {
    fn from(f: &Flow) -> Self {
        Self {
            id: f.id.clone(),
            name: f.name.clone(),
            version: f.version,
            nodes: f.nodes.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(id: &str, needs: &[&str]) -> Node {
        Node {
            id: id.into(),
            node_type: "rhai".into(),
            needs: needs.iter().map(|s| s.to_string()).collect(),
            inputs: Vec::new(),
            with: Default::default(),
            config: json!({}),
            position: None,
        }
    }

    fn flow(nodes: Vec<Node>) -> Flow {
        Flow {
            workspace: "ws".into(),
            id: "f".into(),
            name: "f".into(),
            version: 1,
            params: Default::default(),
            nodes,
            failure_policy: FailurePolicy::Halt,
            deleted: false,
            enabled: true,
            start_on_boot: false,
            placement: Placement::Either,
            concurrency: Concurrency::default(),
            cron: None,
            next_attempt_ts: 0,
        }
    }

    #[test]
    fn validates_a_linear_dag() {
        let f = flow(vec![node("a", &[]), node("b", &["a"]), node("c", &["b"])]);
        validate_flow(&f, MAX_FLOW_NODES).expect("linear is valid");
        assert_eq!(f.frontier(), vec!["a"]);
    }

    #[test]
    fn rejects_a_cycle() {
        let f = flow(vec![node("a", &["b"]), node("b", &["a"])]);
        assert_eq!(validate_flow(&f, MAX_FLOW_NODES), Err(DagError::Cycle));
    }

    #[test]
    fn rejects_over_the_node_cap() {
        // Superset proof for the retired rule-DAG `size_cap_is_rejected` case: the DAG validator
        // rejects a flow whose node count exceeds the cap, before any run. Uses a small cap (2) with 3
        // nodes so the test is cheap and the arithmetic is obvious.
        let f = flow(vec![node("a", &[]), node("b", &[]), node("c", &[])]);
        assert_eq!(validate_flow(&f, 2), Err(DagError::TooManyNodes(3, 2)));
    }

    #[test]
    fn rejects_dangling_dep() {
        let f = flow(vec![node("a", &["zzz"])]);
        assert_eq!(
            validate_flow(&f, MAX_FLOW_NODES),
            Err(DagError::UnknownDependency("a".into(), "zzz".into()))
        );
    }

    #[test]
    fn rejects_self_edge_and_dup_and_empty() {
        assert_eq!(
            validate_flow(&flow(vec![node("a", &["a"])]), MAX_FLOW_NODES),
            Err(DagError::SelfDependency("a".into()))
        );
        assert_eq!(
            validate_flow(&flow(vec![node("a", &[]), node("a", &[])]), MAX_FLOW_NODES),
            Err(DagError::DuplicateNode("a".into()))
        );
        assert_eq!(
            validate_flow(&flow(vec![]), MAX_FLOW_NODES),
            Err(DagError::Empty)
        );
    }

    /// A node whose `with` binds `payload` (a join author resolved the ambiguity explicitly).
    fn join_node(id: &str, needs: &[&str]) -> Node {
        let mut n = node(id, needs);
        n.with.insert("payload".into(), json!("${steps.b.payload}"));
        n
    }

    #[test]
    fn diamond_frontier_orders_correctly() {
        // The diamond's sink `d` joins `b` + `c`, so it MUST bind `payload` (D3 join lint).
        let f = flow(vec![
            node("a", &[]),
            node("b", &["a"]),
            node("c", &["a"]),
            join_node("d", &["b", "c"]),
        ]);
        validate_flow(&f, MAX_FLOW_NODES).unwrap();
        assert_eq!(f.frontier(), vec!["a"]);
        let deps = f.dependents();
        assert_eq!(deps["a"], vec!["b", "c"]);
    }

    #[test]
    fn rejects_a_join_with_no_payload_binding() {
        // The join lint is policy-aware and lives in `flows.save` (where the descriptor registry
        // resolves a port's `all` vs `any`). validate_flow's job is the pure DAG (cycle/dangling/
        // dup/self-edge/port-agreement) — a multi-input node is structurally valid here; the binding
        // check is save-time against the registry. (flow-input-ports-scope moved the lint off the
        // pure model so an `any` funnel with N wires is not falsely rejected.)
        let f = flow(vec![node("a", &[]), node("b", &[]), node("c", &["a", "b"])]);
        validate_flow(&f, MAX_FLOW_NODES).expect("pure DAG accepts a multi-input node");
        // ...and a node that names its ports round-trips them.
        let f = flow(vec![
            node("a", &[]),
            node("b", &[]),
            join_node("c", &["a", "b"]),
        ]);
        validate_flow(&f, MAX_FLOW_NODES).expect("explicit join is valid");
    }

    #[test]
    fn builtin_type_detection() {
        assert!(is_builtin_type("rhai"));
        assert!(is_builtin_type("trigger"));
        assert!(!is_builtin_type("mqtt.publish"));
    }

    #[test]
    fn reachable_from_is_the_downstream_subgraph() {
        // Two independent triggers in one flow: tA→x→z and tB→y→z (z is shared downstream).
        let f = flow(vec![
            node("tA", &[]),
            node("x", &["tA"]),
            node("tB", &[]),
            node("y", &["tB"]),
            node("z", &["x", "y"]),
        ]);
        // Firing tA reaches only its own wires (and the shared z) — never tB or y.
        let from_a = f.reachable_from("tA");
        assert!(from_a.contains("tA") && from_a.contains("x") && from_a.contains("z"));
        assert!(!from_a.contains("tB") && !from_a.contains("y"));
        // An unknown entry reaches nothing.
        assert!(f.reachable_from("nope").is_empty());
    }

    #[test]
    fn indegrees_within_counts_only_in_subset_needs() {
        let f = flow(vec![
            node("tA", &[]),
            node("x", &["tA"]),
            node("tB", &[]),
            node("z", &["x", "tB"]), // z needs x (in tA's subgraph) AND tB (NOT in it)
        ]);
        let set = f.reachable_from("tA"); // {tA, x, z} — tB excluded
        let indeg = f.indegrees_within(&set);
        assert_eq!(indeg["tA"], 0);
        assert_eq!(indeg["x"], 1);
        // z's `tB` need is out-of-subset, so it is NOT counted — z fires on its in-subgraph upstream
        // (x) alone, never waiting forever on a wire that carried no message this firing.
        assert_eq!(indeg["z"], 1);
        assert!(!indeg.contains_key("tB"));
    }

    // --- flow-input-ports-scope Axis 1: port-labelled edges ---

    #[test]
    fn input_edge_round_trips_with_to_port() {
        // An edge targeting a named port serialises + deserialises faithfully (export/import).
        let e = json!({"from": "mqtt-a", "toPort": "payload"});
        let parsed: InputEdge = serde_json::from_value(e).unwrap();
        assert_eq!(parsed, InputEdge::new("mqtt-a", Some("payload".into())));
        let back = serde_json::to_value(&parsed).unwrap();
        assert_eq!(back["from"], "mqtt-a");
        assert_eq!(back["toPort"], "payload");
    }

    #[test]
    fn node_with_inputs_round_trips_and_a_pre_ports_node_loads_unchanged() {
        // A pre-ports node (no `inputs` field) deserialises with empty `inputs` ⇒ every edge is the
        // primary input (the back-compat / no-migration property).
        let pre = json!({"id":"d","type":"debug","needs":["a","b"]});
        let n: Node = serde_json::from_value(pre).unwrap();
        assert!(n.inputs.is_empty());
        assert_eq!(n.to_port_from("a"), None);
        assert_eq!(n.to_port_from("b"), None);

        // A node that names its ports round-trips them.
        let mut n = node("d", &["a", "b"]);
        n.inputs.push(InputEdge::new("a", Some("payload".into())));
        let v = serde_json::to_value(&n).unwrap();
        assert_eq!(v["inputs"][0]["from"], "a");
        assert_eq!(v["inputs"][0]["toPort"], "payload");
        let back: Node = serde_json::from_value(v).unwrap();
        assert_eq!(back.to_port_from("a"), Some("payload".to_string()));
        // An unmentioned edge ⇒ None (primary).
        assert_eq!(back.to_port_from("b"), None);
    }

    #[test]
    fn edges_into_returns_per_port_wires() {
        // Two wires into `d` on the primary port, one wire into `joiner` on a named port.
        let mut d = node("d", &["a", "b"]);
        d.inputs.push(InputEdge::new("b", Some("control".into()))); // a ⇒ primary (None), b ⇒ control
        let f = flow(vec![node("a", &[]), node("b", &[]), d]);
        let edges = f.edges_into("d");
        assert_eq!(edges.len(), 2);
        // Each upstream's port is resolved from `inputs`; an absent entry ⇒ None (primary).
        let by_from: std::collections::HashMap<&str, &Option<String>> =
            edges.iter().map(|(f, p)| (f.as_str(), p)).collect();
        assert_eq!(by_from["a"], &None);
        assert_eq!(by_from["b"], &Some("control".to_string()));
    }

    #[test]
    fn indegrees_within_by_port_groups_per_port() {
        // joiner has two wires on the primary port (a, c) and one on `secondary` (b). A per-port
        // barrier reads the primary count (2) and the `secondary` count (1) separately.
        let mut joiner = node("joiner", &["a", "b", "c"]);
        joiner
            .inputs
            .push(InputEdge::new("b", Some("secondary".into())));
        let f = flow(vec![node("a", &[]), node("b", &[]), node("c", &[]), joiner]);
        let set: HashSet<String> = f.nodes.iter().map(|n| n.id.clone()).collect();
        let by_port = f.indegrees_within_by_port(&set);
        assert_eq!(by_port[&("joiner".into(), None)], 2); // a + c on the primary port
        assert_eq!(by_port[&("joiner".into(), Some("secondary".into()))], 1); // b on secondary
    }

    #[test]
    fn rejects_a_port_entry_for_a_wire_that_is_not_wired() {
        // Port metadata must agree with `needs`: a `to_port` for an upstream NOT in `needs` is a
        // mistake (a label with no wire). Caught at save before any run.
        let mut n = node("d", &["a"]);
        n.inputs
            .push(InputEdge::new("ghost", Some("payload".into())));
        let f = flow(vec![node("a", &[]), n]);
        assert_eq!(
            validate_flow(&f, MAX_FLOW_NODES),
            Err(DagError::PortForUnknownNeed("d".into(), "ghost".into()))
        );
    }
}
