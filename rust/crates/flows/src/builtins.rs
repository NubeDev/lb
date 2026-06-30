//! The five built-in node descriptors (node-descriptor-scope "The five built-in descriptors"). They
//! ship **with the host** but expose the **identical** [`NodeDescriptor`] shape so the editor renders
//! them through the same palette path as extension nodes — one registry, one renderer, no "is this
//! native?" branch. They map onto the spine's node model (`flows-scope.md` "The node model").
//!
//! | `type` | `kind` | `tool` binding | ports | config (shape) |
//! |---|---|---|---|---|
//! | `trigger` | trigger | host (no MCP tool) | out: `fire` | `{ mode, ... }` (cron spec / series / inject sub-mode) |
//! | `tool` | transform | the node's `mcp_verb` config field | in: `args`; out: `output` | `{ verb, args }` |
//! | `rhai` | transform | host `rules.eval` (the lb-rules cage) | in: `input`; out: `output`,`findings` | `{ source }` |
//! | `subflow` | transform | host `flows.run` (child, pinned) | in/out by the child's named ports | `{ flow }` |
//! | `sink` | sink | host write (`inbox\|outbox\|channel\|series`) or an ext-node | in: `value` | `{ target }` |
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

/// The five built-in descriptors, in the one shared shape. The `tool` field for a built-in is a
/// host-internal binding the engine interprets (it is not dispatched as an MCP call the way an
/// extension node's `<ext>.<tool>` is) — `trigger`/`rhai`/`subflow`/`sink` are host-resolved, while
/// the generic `tool` node carries the verb in its **config** and dispatches it under the caller's
/// own cap (everything-is-a-node for actions, "no widening").
pub fn builtin_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // The flow entry node. No inputs; one output port `fire`. Its `mode` config selects the
        // trigger kind (manual/cron/event/inject/boot); `inject` carries the fire|retain sub-mode
        // (Decision 9). The bound `tool` is empty — the host fires it directly, never an MCP call.
        NodeDescriptor::new("trigger", NodeKind::Trigger, "")
            .with_title("Trigger")
            .with_category("Flow")
            .with_ports(vec![], vec!["fire".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["manual", "cron", "event", "inject", "boot"], "default": "manual"},
                        "cron": {"type": "string", "description": "5-field cron spec (mode=cron)"},
                        "series": {"type": "string", "description": "source series to watch (mode=event)"},
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
            .with_ports(vec!["args".into()], vec!["output".into()])
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
        // carry the cage convention: `output` + `findings` (the chain binding grammar verbatim).
        NodeDescriptor::new("rhai", NodeKind::Transform, HOST_RULES_EVAL)
            .with_title("Rhai")
            .with_category("Flow")
            .with_ports(vec!["input".into()], vec!["output".into(), "findings".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["source"],
                    "additionalProperties": false,
                    "properties": {"source": {"type": "string"}}
                }),
            ),
        // A node containing a child graph. Bound to host `flows.run` (a pinned child run the node's
        // step PARKS on, Decision 11). Ports are the child's named ports, mapped by the Decision 4
        // binding grammar — left dynamic (no fixed ports) so a subflow of any shape binds.
        NodeDescriptor::new("subflow", NodeKind::Transform, HOST_FLOWS_RUN)
            .with_title("Subflow")
            .with_category("Flow")
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
            .with_ports(vec!["value".into()], vec![])
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
    fn five_builtins_in_one_shape() {
        let d = builtin_descriptors();
        let types: Vec<&str> = d.iter().map(|x| x.r#type.as_str()).collect();
        assert_eq!(types, vec!["trigger", "tool", "rhai", "subflow", "sink"]);
        // every built-in carries a compilable config schema (load-time contract).
        for desc in &d {
            crate::config_schema::compile_schema(&desc.config)
                .unwrap_or_else(|e| panic!("builtin {} config does not compile: {e}", desc.r#type));
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
}
