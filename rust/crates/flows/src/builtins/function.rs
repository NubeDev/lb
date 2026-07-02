//! The **Function** category descriptors (data-nodes scope): the stateful RBE `filter`, the `unique`
//! dedupe (Data category, but grouped here with the other condition/state nodes), and the two
//! engine-extending nodes `switch` (multi-output conditional routing — edge gating, Decision 14) and
//! `delay` (durable park + rate-limit, reusing the subflow-park suspend/resume, Decision 16). The
//! condition nodes share the [`crate::ops::predicate`] evaluator (Risk 5). Execution lives in the
//! host (state + gating + parking are engine concerns); these are the palette entries.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The four Function/condition/engine descriptors.
pub fn function_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // Report-by-exception (RBE): pass the message only if `payload` changed vs. the last one, or
        // moved more than a deadband. Needs only last-value → the Decision 5 record verbatim.
        NodeDescriptor::new("filter", NodeKind::Transform, "")
            .with_title("Filter (report-by-exception)")
            .with_category("Function")
            .with_icon("funnel")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["changed", "deadband"], "default": "changed", "description": "changed=pass on any change; deadband=pass on numeric move > deadband"},
                        "deadband": {"type": "number", "default": 0, "description": "minimum numeric change to pass (mode=deadband)"},
                        "path": {"type": "string", "description": "field dot-path to compare (absent = whole payload)"}
                    }
                }),
            ),
        // Dedupe. `array` mode (default): drop duplicate elements of an array payload (stateless).
        // `stream` mode: drop a payload already seen across firings (a durable, capped seen-set).
        NodeDescriptor::new("unique", NodeKind::Transform, "")
            .with_title("Unique (dedupe)")
            .with_category("Data")
            .with_icon("fingerprint")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["array", "stream"], "default": "array"},
                        "path": {"type": "string", "description": "field dot-path used as the dedupe key (absent = whole element/payload)"}
                    }
                }),
            ),
        // Multi-output conditional routing (Decision 14, edge gating). Evaluate ordered `rules`
        // against a `property` of the payload; fire only the dependents named in the matched rules'
        // `to` lists. Unmatched dependents' exclusive subtrees are gated (skipped).
        NodeDescriptor::new("switch", NodeKind::Transform, "")
            .with_title("Switch (route)")
            .with_category("Function")
            .with_icon("git-fork")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["rules"],
                    "additionalProperties": false,
                    "properties": {
                        "property": {"type": "string", "description": "field dot-path to test (absent = whole payload)"},
                        "stop_on_first": {"type": "boolean", "default": false, "description": "route to only the first matching rule (else every match)"},
                        "rules": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "required": ["op", "to"],
                                "additionalProperties": false,
                                "properties": {
                                    "op": {"type": "string", "enum": ["eq", "neq", "lt", "lte", "gt", "gte", "contains", "in", "truthy", "falsy", "exists", "missing", "else"]},
                                    "value": {"description": "the operand to compare against"},
                                    "to": {"type": "array", "items": {"type": "string"}, "description": "downstream node ids fired when this rule matches"}
                                }
                            }
                        }
                    }
                }),
            ),
        // Durable delay + rate-limit (Decision 16). `delay` mode: hold the message `ms` then release
        // (parks on a durable timer, resumes after restart — never an in-memory sleep). `rate` mode:
        // release at most one message per `rate_ms` (a durable spacing).
        NodeDescriptor::new("delay", NodeKind::Transform, "")
            .with_title("Delay / rate-limit")
            .with_category("Function")
            .with_icon("timer")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "mode": {"type": "string", "enum": ["delay", "rate"], "default": "delay"},
                        "ms": {"type": "integer", "default": 1000, "minimum": 0, "description": "hold duration (mode=delay)"},
                        "rate_ms": {"type": "integer", "default": 1000, "minimum": 0, "description": "minimum spacing between releases (mode=rate)"}
                    }
                }),
            ),
    ]
}
