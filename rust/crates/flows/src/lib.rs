//! `lb-flows` — the node-graph flow engine's **pure model + descriptor contract** (flows scope,
//! generalised from the shipped `chains` rule-DAG — Decision 6). This crate owns the data the editor
//! renders and the engine schedules; it owns **no** execution, store, bus, or host seam.
//!
//! The thesis (flows-scope): a flow is **not a new engine**. It is a typed node-graph + a backend
//! node-registry run on `lb-jobs`, with state in SurrealDB and motion on Zenoh. This crate fixes the
//! contract both the editor and the engine consume; the run engine + MCP verbs live in the host.
//!
//! Modules (one responsibility per file, FILE-LAYOUT):
//! - [`descriptor`] — the `NodeDescriptor` keystone shape (built-in and extension node alike).
//! - [`node_block`] — the additive `[[node]]` manifest block: parse + validate (`tool` binding,
//!   JSON-Schema config compile) → a `NodeDescriptor`.
//! - [`builtins`] — the five built-in descriptors (trigger / tool / rhai / subflow / sink), in the
//!   identical shape so the palette renders one registry.
//! - [`registry`] — the merged `flows.nodes` registry: built-ins ∪ every installed extension's
//!   validated node blocks (a read-time union, never stored — node-descriptor-scope).
//! - [`config_schema`] — compile + validate a node config against JSON-Schema 2020-12 (Decision 3).
//! - [`model`] — the typed `Flow` graph (`Node`/`Edge`/`needs`/`with`) + DAG math (Kahn cycle-detect,
//!   indegrees/dependents/frontier), reusing the chain binding grammar verbatim.
//! - [`binding`] — resolve a node's `with` bindings (whole-value `${steps.x.output}` /
//!   `${params.y}` / literal) against recorded upstream outputs + flow params — the chain rule,
//!   no templating mini-language.

pub mod binding;
pub mod builtins;
pub mod config_schema;
pub mod descriptor;
pub mod model;
pub mod node_block;
pub mod registry;

pub use binding::resolve_bindings;
pub use builtins::builtin_descriptors;
pub use config_schema::{compile_schema, validate_config, ConfigSchemaError};
pub use descriptor::{NodeDescriptor, NodeKind};
pub use model::{
    validate_flow, DagError, FailurePolicy, Flow, FlowSummary, Node, NodeConfig, BUILTIN_PREFIX,
};
pub use node_block::{validate_node_block, NodeBlock, NodeBlockError};
pub use registry::merge_registry;

/// The SurrealDB tables a flow owns within a workspace namespace (one place owns the names so every
/// verb agrees). The graph record + the run-store records (flow-run-scope "Data").
pub mod table {
    pub const FLOW: &str = "flow";
    pub const FLOW_RUN: &str = "flow_run";
    pub const FLOW_STEP: &str = "flow_step_output";
    pub const FLOW_NODE_STATE: &str = "flow_node_state";
    pub const FLOW_INPUT: &str = "flow_input";
}
