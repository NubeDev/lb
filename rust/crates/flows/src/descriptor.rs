//! The `NodeDescriptor` — the keystone contract (node-descriptor-scope). One shape describes every
//! flow node to **both** the editor (palette + schema-rendered config form) and the engine (the
//! bound tool to dispatch + ports to wire), whether it ships with the host or is contributed by an
//! extension. The editor reads one merged registry and never branches on "is this built-in?".
//!
//! A descriptor carries: an identity (`type`, unique within its origin), human labels (`title`,
//! `category`) for the palette, a coarse [`NodeKind`] (palette grouping + wiring affordances — a
//! `trigger` has no inputs, a `sink` no outputs, a `source` host-arms a series), the **named ports**
//! (`inputs`/`outputs`) that carry the chain binding grammar, the `tool` binding that runs it, and
//! an inline **JSON-Schema 2020-12** config (Decision 3) + a `config_version` for schema evolution.
//!
//! The descriptor **declares no capabilities itself** — reading the catalog reveals only *what could
//! run*. The executing tool's own caps gate actual execution (`caller ∩ install-grant`,
//! extension-nodes-scope). So the palette is broadly readable; the deny lives at run time.

use serde::{Deserialize, Serialize};

/// The coarse class the editor groups by + uses for wiring affordances. It does **not** pick the
/// runner — the bound `tool` does. Mirrors the `kind` field of the `[[node]]` manifest block.
///
/// - `Trigger` — a flow entry node; no inputs, host-fired (manual/cron/event/inject/boot).
/// - `Transform` — request→response: takes inputs, returns a derived value (the generic `tool` node,
///   the `rhai` cage node, the `subflow` node).
/// - `Sink` — a terminal node with no outputs; a must-deliver sink stages an outbox effect.
/// - `Source` — long-lived external feed; the host **arms** (allocates a series + starts it) on flow
///   enable and **disarms** on disable (Decision 2). The event-trigger watches its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeKind {
    Trigger,
    Transform,
    Sink,
    Source,
}

/// One node descriptor — the join between the editor and the engine. Built-in descriptors are
/// synthesised by the host ([`crate::builtins`]); extension descriptors come from a validated
/// `[[node]]` block ([`crate::node_block`]). The two are the same shape on purpose (Decision:
/// one registry, one renderer, no `if native` branch).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeDescriptor {
    /// The globally-unique node type: a built-in (`trigger`/`tool`/`rhai`/`subflow`/`sink`) or
    /// `<ext_id>.<type>` for an extension node (the ext-unique `type` namespaced by `ext_id`).
    #[serde(rename = "type")]
    pub r#type: String,
    /// Palette + node-header label. Defaults to `type` when a manifest omits it.
    pub title: String,
    /// Palette group. Defaults to `"General"`.
    pub category: String,
    /// A lucide icon name the palette + node render (e.g. `"zap"`, `"hash"`). `None` → the editor
    /// falls back by `kind`. Built-ins carry one; an extension `[[node]]` may declare `icon`
    /// (persisted on the install) and otherwise gets the kind fallback. Pure palette chrome — the
    /// engine never reads it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Coarse class for palette grouping + wiring affordances.
    pub kind: NodeKind,
    /// The MCP tool this node dispatches when it runs. For a built-in this is a host-internal
    /// binding (e.g. `rules.eval`, `flows.run`); for an extension node it is `<ext_id>.<tool>`.
    /// The engine calls it under `caller ∩ install-grant` (no widening).
    pub tool: String,
    /// Named input ports (edges land here). A `trigger`/`source` has none.
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Named output ports (edges leave here). A `sink` has none.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Bumped when the config schema changes shape (the job `schema_version` discipline). A run
    /// pins the flow version (Decision 1); a version bump at save re-validates persisted configs
    /// against the new schema (node-descriptor-scope "config_version + evolution").
    pub config_version: u32,
    /// Inline JSON-Schema 2020-12 the editor renders the settings form from + the host validates a
    /// node's saved config against (Decision 3). `{}` (accept anything) when a node has no config.
    #[serde(default = "default_config")]
    pub config: serde_json::Value,
}

fn default_config() -> serde_json::Value {
    serde_json::json!({})
}

impl NodeDescriptor {
    /// Build a descriptor, applying the manifest defaults (`title` ← `type`, `category` ←
    /// `"General"`, ports ← empty, `config_version` ← 1, `config` ← `{}`).
    pub fn new(r#type: impl Into<String>, kind: NodeKind, tool: impl Into<String>) -> Self {
        let r#type = r#type.into();
        Self {
            title: r#type.clone(),
            category: "General".into(),
            icon: None,
            r#type,
            kind,
            tool: tool.into(),
            inputs: Vec::new(),
            outputs: Vec::new(),
            config_version: 1,
            config: default_config(),
        }
    }

    /// Builder: set the palette title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    /// Builder: set the palette category.
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = category.into();
        self
    }
    /// Builder: set the palette icon (a lucide icon name).
    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
    /// Builder: set the named ports.
    pub fn with_ports(mut self, inputs: Vec<String>, outputs: Vec<String>) -> Self {
        self.inputs = inputs;
        self.outputs = outputs;
        self
    }
    /// Builder: set the config schema + version.
    pub fn with_config(mut self, config_version: u32, config: serde_json::Value) -> Self {
        self.config_version = config_version;
        self.config = config;
        self
    }
}
