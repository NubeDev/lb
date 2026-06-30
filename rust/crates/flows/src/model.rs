//! The typed node-graph `Flow` model + DAG math (flows-scope "The node model", generalised from the
//! chain `Step`). A flow is a validated DAG whose every node is data-driven: an `id`, a `node_type`
//! referencing a [`crate::descriptor::NodeDescriptor`] (built-in or `<ext>.<type>`), a `config`
//! validated against that descriptor's schema, and `needs` + `with` carrying the **chain binding
//! grammar verbatim** — whole-value `${steps.x.output}` / `${params.y}` references or a literal, no
//! templating mini-language (rule-chains-scope, lifted verbatim).
//!
//! The DAG math (Kahn cycle-detect, indegrees/dependents/frontier) mirrors the chain `Chain::validate`
//! exactly — a flow **is** the generalised chain topology with a typed node payload (Decision 8). It
//! is pure math with no I/O, so a flow is validated at save before any node runs.

use std::collections::{HashMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::descriptor::NodeKind;

/// The size cap on a flow's node count (mirrors the chain cap; re-checks a hand-edited record).
pub const MAX_FLOW_NODES: usize = 256;

/// No extension namespace prefix — built-in types (`trigger`/`tool`/`rhai`/`subflow`/`sink`) carry
/// no `<ext_id>.` prefix; an extension node's type is always `<ext_id>.<type>`.
pub const BUILTIN_PREFIX: &str = "";

/// Whether a node type is a built-in (no `<ext_id>.` namespace).
pub fn is_builtin_type(node_type: &str) -> bool {
    !node_type.contains('.')
}

/// What happens when a node fails (after its retries): the chain policy verbatim. `Halt` prunes the
/// failed node's transitive subtree (those nodes `Skipped`); `Continue` releases dependents with the
/// failed output resolved to `null`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FailurePolicy {
    #[default]
    Halt,
    Continue,
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
    #[serde(default)]
    pub needs: Vec<String>,
    /// Input bindings: literal | `${steps.x.output}` | `${steps.x.findings}` | `${params.y}`.
    #[serde(default)]
    pub with: serde_json::Map<String, Value>,
    /// The node's config, validated against its descriptor's schema at save.
    #[serde(default)]
    pub config: Value,
}

impl Node {
    pub fn new(id: impl Into<String>, node_type: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            node_type: node_type.into(),
            needs: Vec::new(),
            with: serde_json::Map::new(),
            config: Value::Null,
        }
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
        }
    }
    /// In-degree (number of `needs`) per node — the chain math verbatim.
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

/// A DAG validation error (the chain `DagError` shapes verbatim). All rejected **before any node
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
}

/// Validate a flow's DAG: non-empty, within `max_nodes`, unique ids, deps resolve, no self-edge,
/// acyclic (Kahn). Reused verbatim from the chain `Chain::validate`.
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
            with: Default::default(),
            config: json!({}),
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

    #[test]
    fn diamond_frontier_orders_correctly() {
        let f = flow(vec![
            node("a", &[]),
            node("b", &["a"]),
            node("c", &["a"]),
            node("d", &["b", "c"]),
        ]);
        validate_flow(&f, MAX_FLOW_NODES).unwrap();
        assert_eq!(f.frontier(), vec!["a"]);
        let deps = f.dependents();
        assert_eq!(deps["a"], vec!["b", "c"]);
    }

    #[test]
    fn builtin_type_detection() {
        assert!(is_builtin_type("rhai"));
        assert!(is_builtin_type("trigger"));
        assert!(!is_builtin_type("mqtt.publish"));
    }
}
