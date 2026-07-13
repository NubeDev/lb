//! The `NodeDescriptor` — the keystone contract (node-descriptor-scope). One shape describes every
//! flow node to **both** the editor (palette + schema-rendered config form) and the engine (the
//! bound tool to dispatch + ports to wire), whether it ships with the host or is contributed by an
//! extension. The editor reads one merged registry and never branches on "is this built-in?".
//!
//! A descriptor carries: an identity (`type`, unique within its origin), human labels (`title`,
//! `category`) for the palette, a coarse [`NodeKind`] (palette grouping + wiring affordances — a
//! `trigger` has no inputs, a `sink` no outputs, a `source` host-arms a series), the **named ports**
//! (`inputs`/`outputs`) that carry the rubix-cube binding grammar, the `tool` binding that runs it, and
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
    /// Named input ports (edges land here). A `trigger`/`source` has none. The simple string list
    /// (the port names); the per-port **join policy** rides the parallel [`Self::input_ports`]
    /// table — when that is empty, every port here defaults to [`JoinPolicy::Any`] (plain
    /// per-message wiring, the Node-RED model; flow-plain-wiring-scope).
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Named output ports (edges leave here). A `sink` has none.
    #[serde(default)]
    pub outputs: Vec<String>,
    /// Per-input-port **join policy** table. Additive + serde-defaulted: an empty table means every
    /// port in [`Self::inputs`] is [`JoinPolicy::Any`] (plain per-message wiring — the universal
    /// default, flow-plain-wiring-scope). An entry is a descriptor-level **opt-in**: an extension
    /// node may declare `join = "all"` on a port to get the barrier (fire once when every wired
    /// upstream settles). No built-in declares it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub input_ports: Vec<InputPort>,
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

/// The per-input-port **join policy** (flow-input-ports-scope Axis 2, default flipped by
/// flow-plain-wiring-scope). Declares how a port combines the wires that land on it.
///
/// - `Any` — a **funnel** (Node-RED's fire-per-message OR) — **the universal default**. The node is
///   released **once per settled upstream** on that port, each firing carrying that one upstream's
///   envelope. Multiplicity is statically bounded by the wire topology (path count), never by event
///   volume. Plain wiring is the whole story: no built-in declares anything else.
/// - `All` — a **barrier**, a descriptor-level **opt-in** (an extension `[[node.input]]` may declare
///   `join = "all"`). The node fires **once** when every wired upstream on that port has settled;
///   the port's value is combined by an explicit binding (the save lint demands one).
///
/// The serde `Default` derive still needs a variant marker; the *effective* default every resolver
/// applies ([`NodeDescriptor::join_of`], the host run-store fallback, the UI `joinOf` mirror) is
/// `Any` — an `[[node.input]]` entry that omits `join` means `any` too.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JoinPolicy {
    #[default]
    Any,
    All,
}

/// One declared input port with its join policy (the `[[node.input]]` manifest table form). `name`
/// matches an entry in [`NodeDescriptor::inputs`]; `join` defaults to [`JoinPolicy::Any`] when
/// omitted — the same per-message default the string `inputs = [...]` shorthand means. `all` is the
/// explicit opt-in (flow-plain-wiring-scope).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputPort {
    pub name: String,
    #[serde(default)]
    pub join: JoinPolicy,
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
            input_ports: Vec::new(),
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
    /// Builder: set the per-input-port join-policy table (flow-input-ports-scope Axis 2). Ports not
    /// listed default to [`JoinPolicy::All`].
    pub fn with_input_ports(mut self, input_ports: Vec<InputPort>) -> Self {
        self.input_ports = input_ports;
        self
    }
    /// Builder: set the config schema + version.
    pub fn with_config(mut self, config_version: u32, config: serde_json::Value) -> Self {
        self.config_version = config_version;
        self.config = config;
        self
    }

    /// The **primary** input port name — the port an edge with no `to_port` lands on (flow-input-
    /// ports-scope Axis 1: omitted `to_port` ⇒ the first declared input). `None` when the node
    /// declares no input ports (a `trigger`/`source`).
    pub fn primary_input(&self) -> Option<&str> {
        self.inputs.first().map(|s| s.as_str())
    }

    /// The declared [`JoinPolicy`] for `port`. **Every port defaults to `any`** — plain per-message
    /// wiring, for every node kind (flow-plain-wiring-scope; there is deliberately no per-kind
    /// branch left to re-grow policy-by-kind). An explicit `input_ports` entry declaring `all` is
    /// the only way a port barriers. `port = None` resolves the primary input port.
    pub fn join_of(&self, port: Option<&str>) -> JoinPolicy {
        let name = match port {
            Some(p) if !p.is_empty() => p,
            _ => match self.primary_input() {
                Some(p) => p,
                None => return JoinPolicy::Any,
            },
        };
        self.input_ports
            .iter()
            .find(|ip| ip.name == name)
            .map(|ip| ip.join)
            .unwrap_or(JoinPolicy::Any)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc(kind: NodeKind, inputs: Vec<&str>) -> NodeDescriptor {
        NodeDescriptor::new("x", kind, "")
            .with_ports(inputs.into_iter().map(String::from).collect(), vec![])
    }

    #[test]
    fn join_defaults_any_for_every_kind() {
        // flow-plain-wiring-scope: EVERY port defaults to `any` — plain per-message wiring, no
        // per-kind branch. Three wires into a transform behaves exactly like Node-RED.
        for kind in [
            NodeKind::Transform,
            NodeKind::Sink,
            NodeKind::Trigger,
            NodeKind::Source,
        ] {
            let d = desc(kind, vec!["payload"]);
            assert_eq!(d.join_of(None), JoinPolicy::Any, "{kind:?} primary");
            assert_eq!(d.join_of(Some("payload")), JoinPolicy::Any, "{kind:?}");
            // A port not declared on the node still resolves the default.
            assert_eq!(d.join_of(Some("nope")), JoinPolicy::Any, "{kind:?} unknown");
        }
        // A node with no inputs at all still answers `any` (nothing to barrier on).
        assert_eq!(
            desc(NodeKind::Trigger, vec![]).join_of(None),
            JoinPolicy::Any
        );
    }

    #[test]
    fn input_ports_table_opts_a_port_into_all() {
        // The `all` barrier survives as a descriptor-level opt-in (an extension may declare it);
        // it is never a default and no built-in declares it.
        let d = desc(NodeKind::Transform, vec!["payload"]).with_input_ports(vec![InputPort {
            name: "payload".into(),
            join: JoinPolicy::All,
        }]);
        assert_eq!(d.join_of(None), JoinPolicy::All); // None ⇒ primary, which is opted in
        assert_eq!(d.join_of(Some("payload")), JoinPolicy::All);
        // A port not in the table still defaults to `any`.
        assert_eq!(d.join_of(Some("other")), JoinPolicy::Any);
        // An entry that (redundantly) declares `any` is honoured too.
        let s = desc(NodeKind::Sink, vec!["payload"]).with_input_ports(vec![InputPort {
            name: "payload".into(),
            join: JoinPolicy::Any,
        }]);
        assert_eq!(s.join_of(Some("payload")), JoinPolicy::Any);
    }

    #[test]
    fn primary_input_is_the_first_declared_port() {
        assert_eq!(
            desc(NodeKind::Transform, vec!["payload"]).primary_input(),
            Some("payload")
        );
        // A multi-port node: the first is primary (the edge default target).
        let d = desc(NodeKind::Transform, vec!["left", "right"]);
        assert_eq!(d.primary_input(), Some("left"));
        // A node with no inputs (trigger/source) has no primary — an inbound wire is rejected at save.
        assert_eq!(desc(NodeKind::Trigger, vec![]).primary_input(), None);
    }

    #[test]
    fn descriptor_with_input_ports_round_trips() {
        let d = desc(NodeKind::Sink, vec!["payload"]).with_input_ports(vec![InputPort {
            name: "payload".into(),
            join: JoinPolicy::Any,
        }]);
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["inputPorts"][0]["name"], "payload");
        assert_eq!(v["inputPorts"][0]["join"], "any");
        // An empty table is skipped (the clean wire shape for the common all-All case).
        let plain = desc(NodeKind::Transform, vec!["payload"]);
        assert!(serde_json::to_value(&plain)
            .unwrap()
            .get("inputPorts")
            .is_none());
    }
}
