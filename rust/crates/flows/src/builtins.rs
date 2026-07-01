//! The built-in node descriptors (node-descriptor-scope "The built-in descriptors"). They ship
//! **with the host** but expose the **identical** [`NodeDescriptor`] shape so the editor renders them
//! through the same palette path as extension nodes — one registry, one renderer, no "is this
//! native?" branch. They map onto the spine's node model (`flows-scope.md` "The node model").
//!
//! Every built-in speaks the **message envelope** (flow-message-envelope-scope D6): its input port is
//! `payload` (the value) with `topic` carried alongside, and its output is `payload` (+ any field it
//! sets, e.g. `topic`/`findings`). Ports are named `payload`/`topic` so the palette, canvas handles,
//! and dashboard picker all speak one vocabulary.
//!
//! | `type` | `kind` | `tool` binding | ports | config (shape) |
//! |---|---|---|---|---|
//! | `trigger` | trigger | host (no MCP tool) | out: `payload`,`topic` | `{ mode, topic?, ... }` |
//! | `tool` | transform | the node's `mcp_verb` config field | in: `payload`; out: `payload` | `{ verb, args }` |
//! | `rhai` | transform | host `rules.eval` (the lb-rules cage) | in: `payload`; out: `payload`,`topic`,`findings` | `{ source }` |
//! | `count` | transform | host (no MCP tool) | in: `payload`; out: `payload` | `{}` (counts its input) |
//! | `counter` | transform | host (no MCP tool) | in: `payload`; out: `payload` | `{ mode, step, reset }` |
//! | `subflow` | transform | host `flows.run` (child, pinned) | in: `payload`; out: `payload` | `{ flow }` |
//! | `sink` | sink | host write (`inbox\|outbox\|channel\|series`) or an ext-node | in: `payload`,`topic` | `{ target }` |
//!
//! The `trigger` node's `inject` sub-mode (Decision 9) splits intent: `fire` starts a one-shot run
//! with the value; `retain` updates the node's retained `flow_input` value and starts no run. The
//! `subflow` node's `flows.run` binding **parks on the pinned child run** (Decision 11) — the
//! coordination detail is `flow-run-scope.md`; here we declare only its descriptor.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The host-side tool bindings for built-ins (the `tool` field). `BUILTIN_PREFIX` is the type prefix
/// the registry uses to recognise a built-in (no `<ext_id>.` namespace).
const HOST_RULES_EVAL: &str = "rules.eval";
const HOST_FLOWS_RUN: &str = "flows.run";

/// The built-in descriptors, in the one shared shape. The `tool` field for a built-in is a
/// host-internal binding the engine interprets (it is not dispatched as an MCP call the way an
/// extension node's `<ext>.<tool>` is) — `trigger`/`rhai`/`subflow`/`sink`/`count` are host-resolved,
/// while the generic `tool` node carries the verb in its **config** and dispatches it under the
/// caller's own cap (everything-is-a-node for actions, "no widening").
pub fn builtin_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // The flow entry node. No inputs; one output port `fire`. Its `mode` config selects the
        // trigger kind (manual/cron/event/inject/boot); `inject` carries the fire|retain sub-mode
        // (Decision 9). The bound `tool` is empty — the host fires it directly, never an MCP call.
        NodeDescriptor::new("trigger", NodeKind::Trigger, "")
            .with_title("Trigger")
            .with_category("Flow")
            .with_icon("zap")
            .with_ports(vec![], vec!["payload".into(), "topic".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["manual", "cron", "event", "inject", "boot"], "default": "manual"},
                        "cron": {"type": "string", "description": "5-field cron spec (mode=cron)"},
                        "series": {"type": "string", "description": "source series to watch (mode=event)"},
                        "topic": {"type": "string", "description": "the topic stamped on the firing envelope (D6)"},
                        "inject_mode": {"type": "string", "enum": ["fire", "retain"], "default": "fire", "description": "Decision 9 (mode=inject)"}
                    }
                }),
            ),
        // Everything-is-a-node for ACTIONS (not entities). Carries the granted MCP verb + args in
        // its config; the engine dispatches it under the caller's own cap (caller ∩ grant) — the
        // generic tool node is why the registry needs no descriptor per verb.
        NodeDescriptor::new("tool", NodeKind::Transform, "")
            .with_title("Tool")
            .with_category("Flow")
            .with_icon("wrench")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["verb"],
                    "additionalProperties": false,
                    "properties": {
                        "verb": {"type": "string", "description": "the granted MCP verb to dispatch"},
                        "args": {"type": "object", "default": {}}
                    }
                }),
            ),
        // The function node — the lb-rules rhai cage. Bound to the host `rules.eval` tool. Ports
        // carry the cage convention: `output` + `findings` (the rubix-cube binding grammar verbatim).
        NodeDescriptor::new("rhai", NodeKind::Transform, HOST_RULES_EVAL)
            .with_title("Rhai")
            .with_category("Flow")
            .with_icon("code")
            .with_ports(
                vec!["payload".into()],
                vec!["payload".into(), "topic".into(), "findings".into()],
            )
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["source"],
                    "additionalProperties": false,
                    "properties": {"source": {"type": "string"}}
                }),
            ),
        // A pure transform that counts its input: an array → its length, an object → its key count,
        // null → 0, a scalar → 1. Output port `count` carries the integer. No MCP tool — the host
        // resolves it directly (like `trigger`). The plain "how many?" node for a flow.
        NodeDescriptor::new("count", NodeKind::Transform, "")
            .with_title("Count (input size)")
            .with_category("Flow")
            .with_icon("hash")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {}
                }),
            ),
        // The Node-RED `json` node: convert the `payload` between a JSON STRING and a structured value
        // at a flow's text boundary (a webhook body, an MQTT text message, a file read) — the one thing
        // neither `rhai` (reshapes an already-structured msg) nor a `${steps.x.payload.field}` binding
        // (walks an already-structured msg) can do. `mode=parse` (default): a JSON string `payload` →
        // its parsed value; a non-string or invalid JSON FAILS the node (Node-RED parity — surfaces a
        // bad body instead of silently flowing a wrong shape). `mode=stringify`: any `payload` → its
        // JSON string (`pretty` indents). Stateless, no MCP tool — the host resolves it like `count`.
        NodeDescriptor::new("json", NodeKind::Transform, "")
            .with_title("JSON (parse / stringify)")
            .with_category("Flow")
            .with_icon("braces")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["parse", "stringify"], "default": "parse", "description": "parse=JSON string→value (fails on bad JSON); stringify=value→JSON string"},
                        "pretty": {"type": "boolean", "default": false, "description": "indent the output string (mode=stringify)"}
                    }
                }),
            ),
        // A STATEFUL accumulator — the Node-RED / PLC counter (the "rung holds its last result").
        // Unlike `count` (a pure transform of THIS firing's input), `counter` reads its own durable
        // last value and increments on every firing, so the value GOES UP across runs and survives a
        // restart. `mode` is EXPLICIT (flow-message-envelope-scope D7, the trap removed): `tick`
        // (default) → +`step` every firing regardless of payload; `throughput` → +the size of the
        // `payload`. `reset` zeroes it. Output `payload` carries the running total. No MCP tool.
        NodeDescriptor::new("counter", NodeKind::Transform, "")
            .with_title("Counter (running total)")
            .with_category("Flow")
            .with_icon("plus")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["tick", "throughput"], "default": "tick", "description": "tick=+step every firing; throughput=+payload size (D7)"},
                        "step": {"type": "integer", "default": 1, "description": "increment per firing (mode=tick)"},
                        "reset": {"type": "boolean", "default": false, "description": "zero the running total before applying this firing"}
                    }
                }),
            ),
        // A node containing a child graph. Bound to host `flows.run` (a pinned child run the node's
        // step PARKS on, Decision 11). Ports are the child's named ports, mapped by the Decision 4
        // binding grammar — left dynamic (no fixed ports) so a subflow of any shape binds.
        NodeDescriptor::new("subflow", NodeKind::Transform, HOST_FLOWS_RUN)
            .with_title("Subflow")
            .with_category("Flow")
            .with_icon("git-branch")
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["flow"],
                    "additionalProperties": false,
                    "properties": {"flow": {"type": "string", "description": "flow-id@version (Decision 4)"}}
                }),
            ),
        // A terminal node. No outputs; one input `value`. Its `target` config selects the host write
        // seam (`inbox`/`outbox`/`channel`/`series`) or an extension sink node. A must-deliver sink
        // stages an outbox effect (transactional, idempotent) — never raw pub/sub.
        NodeDescriptor::new("sink", NodeKind::Sink, "")
            .with_title("Sink")
            .with_category("Flow")
            .with_icon("arrow-down-to-line")
            .with_ports(vec!["payload".into(), "topic".into()], vec![])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["target"],
                    "additionalProperties": false,
                    "properties": {
                        "target": {"type": "string", "enum": ["inbox", "outbox", "channel", "series"]},
                        "name": {"type": "string", "description": "the channel / series name"}
                    }
                }),
            ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_in_one_shape() {
        let d = builtin_descriptors();
        let types: Vec<&str> = d.iter().map(|x| x.r#type.as_str()).collect();
        assert_eq!(
            types,
            vec!["trigger", "tool", "rhai", "count", "json", "counter", "subflow", "sink"]
        );
        // every built-in carries a compilable config schema (load-time contract).
        for desc in &d {
            crate::config_schema::compile_schema(&desc.config)
                .unwrap_or_else(|e| panic!("builtin {} config does not compile: {e}", desc.r#type));
        }
        // every built-in carries a palette icon.
        for desc in &d {
            assert!(desc.icon.is_some(), "builtin {} has no icon", desc.r#type);
        }
    }

    #[test]
    fn trigger_has_no_inputs_sink_has_no_outputs() {
        let d = builtin_descriptors();
        let trig = d.iter().find(|x| x.r#type == "trigger").unwrap();
        let sink = d.iter().find(|x| x.r#type == "sink").unwrap();
        assert!(trig.inputs.is_empty());
        assert!(sink.outputs.is_empty());
    }

    #[test]
    fn count_is_a_pure_transform() {
        let d = builtin_descriptors();
        let count = d.iter().find(|x| x.r#type == "count").unwrap();
        assert_eq!(count.kind, NodeKind::Transform);
        assert_eq!(count.inputs, vec!["payload".to_string()]);
        assert_eq!(count.outputs, vec!["payload".to_string()]);
    }

    #[test]
    fn builtins_speak_the_envelope_ports() {
        // Every built-in's ports are envelope ports — no stray `items`/`value`/`output` literals (D6).
        let d = builtin_descriptors();
        for desc in &d {
            for p in desc.inputs.iter().chain(desc.outputs.iter()) {
                assert!(
                    matches!(p.as_str(), "payload" | "topic" | "findings"),
                    "builtin {} has non-envelope port {p}",
                    desc.r#type
                );
            }
        }
    }
}
