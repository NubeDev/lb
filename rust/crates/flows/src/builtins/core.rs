//! The **original** built-in descriptors (the flow spine): `trigger` / `tool` / `rhai` / `rule` /
//! `count` / `json` / `counter` / `subflow` / `sink`. They ship **with the host** but wear the
//! identical [`NodeDescriptor`] shape as extension nodes — one registry, one renderer, no "is this
//! native?" branch (node-descriptor-scope). The data/JSON node pack ([`super::data`] / [`super::parse`]
//! / [`super::sequence`] / [`super::function`]) adds twenty more in this same mould.
//!
//! Every built-in speaks the **message envelope** (flow-message-envelope-scope D6): input port
//! `payload` (+ `topic` carried alongside), output `payload` (+ any field it sets). Ports are named
//! `payload`/`topic` so palette, canvas handles, and dashboard picker speak one vocabulary.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The host-side tool bindings for built-ins (the `tool` field is a host-internal binding, not an MCP
/// call — see the module doc on [`super`]).
const HOST_RULES_EVAL: &str = "rules.eval";
const HOST_FLOWS_RUN: &str = "flows.run";

/// The eight spine built-ins, in the one shared shape. `trigger`/`rhai`/`subflow`/`sink`/`count`/
/// `json`/`counter` are host-resolved; the generic `tool` node carries its verb in **config** and
/// dispatches under the caller's own cap (everything-is-a-node for actions, "no widening").
pub fn core_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // The flow entry node. No inputs; envelope out. `mode` selects the trigger kind; `inject`
        // carries the fire|retain sub-mode (Decision 9). Empty `tool` — the host fires it directly.
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
        // A self-driving boolean OSCILLATOR (a square-wave source). No inputs; envelope out. Fires on
        // its own durable interval clock (the reactor, like `cron`) and FLIPS its output each firing:
        // `start`, `!start`, `start`, … Holds each value for `period_secs` (default 10s → 10s true / 10s
        // false). A stateful trigger — the durable cursor holds both the clock AND the last value; no
        // input feeds it. No MCP tool.
        NodeDescriptor::new("flipflop", NodeKind::Trigger, "")
            .with_title("Flip-flop (oscillator)")
            .with_category("Flow")
            .with_icon("toggle-left")
            .with_ports(vec![], vec!["payload".into(), "topic".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "period_secs": {"type": "integer", "minimum": 1, "default": 10, "description": "how long each value is held before it flips, in seconds (10 → 10s true / 10s false)"},
                        "start": {"type": "boolean", "default": true, "description": "the value emitted on the first firing"},
                        "topic": {"type": "string", "description": "the topic stamped on the firing envelope (D6)"}
                    }
                }),
            ),
        // The webhook SOURCE node (rules-workflow-convergence scope, slice 5): an entry node whose
        // config is just `{webhook_id}` (a picker over `webhook.list`). It owns no endpoint/credential
        // — the core webhook service owns the hook + its series `webhook:{ws}:{id}`. When the flow
        // enables, the series-event reactor watches that series and fires one run per hit, the hit's
        // payload as the envelope. The ONLY flow-facing inbound surface (no provider-named node).
        NodeDescriptor::new("webhook", NodeKind::Trigger, "")
            .with_title("Webhook")
            .with_category("Flow")
            .with_icon("webhook")
            .with_ports(vec![], vec!["payload".into(), "topic".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["webhook_id"],
                    "additionalProperties": false,
                    "properties": {
                        "webhook_id": {"type": "string", "description": "the webhook this source fires on (webhook.list)"}
                    }
                }),
            ),
        // Everything-is-a-node for ACTIONS: carries the granted MCP verb + args in config; dispatched
        // under the caller's own cap (caller ∩ grant) — one generic descriptor covers every verb.
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
                        "args": {"type": "object", "default": {}},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "wall-clock ceiling for this node's dispatch, in ms (settles err:\"timeout\" if exceeded)"}
                    }
                }),
            ),
        // The function node — the lb-rules rhai cage. Bound to host `rules.eval` (the flow-facing rule
        // entry: message envelope in, findings out). Out carries the cage convention
        // `payload`/`topic`/`findings`. `timeout_ms` overrides the cage wall-clock deadline for this
        // node (rules-workflow-convergence scope, slice 2).
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
                    "properties": {
                        "source": {"type": "string"},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "override the rule cage's wall-clock deadline for this node, in ms"}
                    }
                }),
            ),
        // Run a SAVED rule by name (`rule` = the stored rule id) with the message envelope as params
        // plus any fixed `params`. Bound to host `rules.eval` like `rhai` — the only difference is the
        // rule is selected by id, not inlined (rules-workflow-convergence scope, slice 1).
        NodeDescriptor::new("rule", NodeKind::Transform, HOST_RULES_EVAL)
            .with_title("Rule (saved)")
            .with_category("Flow")
            .with_icon("scroll")
            .with_ports(
                vec!["payload".into()],
                vec!["payload".into(), "topic".into(), "findings".into()],
            )
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["rule"],
                    "additionalProperties": false,
                    "properties": {
                        "rule": {"type": "string", "description": "the saved rule id (rules.list)"},
                        "params": {"type": "object", "default": {}, "description": "fixed params merged over the message envelope"},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "override the rule cage's wall-clock deadline for this node, in ms"}
                    }
                }),
            ),
        // A pure transform: count the input `payload` (array length / object keys / scalar→1). No MCP
        // tool. The plain "how many?" node; for a running total use `counter`.
        NodeDescriptor::new("count", NodeKind::Transform, "")
            .with_title("Count (input size)")
            .with_category("Flow")
            .with_icon("hash")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({"type": "object", "additionalProperties": false, "properties": {}}),
            ),
        // The Node-RED `json` node: convert `payload` between a JSON STRING and a structured value at a
        // text boundary. `parse` (default): string→value (invalid JSON FAILS the node — parity);
        // `stringify`: value→JSON string (`pretty` indents). Stateless, host-resolved.
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
        // A STATEFUL accumulator (Node-RED / PLC counter): reads its own durable last value and
        // increments every firing, surviving restart. `mode` explicit (D7): `tick`→+step per firing;
        // `throughput`→+payload size. `reset` zeroes it. No MCP tool.
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
        // A node containing a child graph. Bound to host `flows.run` (a pinned child run the node PARKS
        // on, Decision 11). No fixed ports — a subflow of any shape binds by the child's named ports.
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
                    "properties": {
                        "flow": {"type": "string", "description": "flow-id@version (Decision 4)"},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "wall-clock ceiling for the child run, in ms (settles err:\"timeout\" if exceeded)"}
                    }
                }),
            ),
        // The approval GATE: park the run until a reviewer approves. Passes the envelope through on
        // approval; fails on reject. Writes a `needs:approval` inbox item routed to `team`; the
        // flow-approval reactor resumes the parked run on resolution (rules-workflow-convergence scope).
        NodeDescriptor::new("approval", NodeKind::Transform, "")
            .with_title("Approval gate")
            .with_category("Flow")
            .with_icon("shield-check")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "team": {"type": "string", "description": "the team the approval item routes to"},
                        "channel": {"type": "string", "description": "the inbox channel for the approval item (default \"approvals\")"}
                    }
                }),
            ),
        // A terminal node. No outputs; envelope in. `target` selects the host write seam
        // (`inbox`/`outbox`/`channel`/`series`) or an ext sink; a must-deliver sink stages an outbox
        // effect (transactional, idempotent) — never raw pub/sub.
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
                        "name": {"type": "string", "description": "the channel / series name"},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "wall-clock ceiling for this node's dispatch, in ms (settles err:\"timeout\" if exceeded)"}
                    }
                }),
            ),
    ]
}
