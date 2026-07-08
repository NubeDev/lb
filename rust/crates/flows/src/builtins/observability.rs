//! The **observability** built-in descriptors — the Node-RED observability-node pack this scope
//! ships first (debug-node-scope). Sibling to [`super::core`] / [`super::data`] / [`super::parse`] /
//! [`super::sequence`] / [`super::function`], in the one shared [`NodeDescriptor`] shape.
//!
//! v1 ships the single `debug` node — Node-RED's debug node: a `sink` that publishes each wire
//! message as **motion** onto a workspace-walled bus subject (fire-and-forget, no SurrealDB record —
//! rule 3 made literal), consumed by the canvas's debug panel over a gateway SSE route. The
//! `catch`/`status`/`complete`/`link` siblings stay deferred to follow-up scopes (this pack's
//! defer-list, named in `data-nodes-scope.md` + `flow-context-scope.md`).
//!
//! Every built-in speaks the **message envelope** (flow-message-envelope-scope D6): input port
//! `payload`. The `debug` node is a terminal observer — one input, no output, so it never gates a
//! subtree (removing it changes only what the panel sees, never what the flow does).

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The default per-node publish governor (max real debug messages per second before a `dropped`
/// sentinel is published instead). Overrideable per-node via `config.rate_limit`. A best-effort
/// guard against a hot source flooding the bus + every open panel — debug is motion, not a reliable
/// log (debug-node-scope Risk 1).
pub const DEFAULT_RATE_LIMIT: u64 = 50;

/// The default long-content collapse threshold (bytes). The panel renders a value larger than this
/// collapsed with a "show more" disclosure; `0` disables collapse. The full value is always on the
/// wire — collapse is presentation only, never truncation (debug-node-scope Decision 6).
pub const DEFAULT_COLLAPSE_BYTES: u64 = 2048;

/// The v1 observability pack: the `debug` node. Node-RED's debug node over the shipped plane — a
/// host-resolved `sink` that runs under `flows.run` (no new execution cap), publishing each wire
/// message as motion onto `flow_debug:{ws}:{flow}` for the debug panel to tail.
pub fn observability_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // Node-RED's debug node. A terminal observer: one `payload` in, no out. Reads the wire
        // envelope's primary slot, resolves its content `format`, and publishes a debug message onto
        // the per-flow debug subject (fire-and-forget motion). The panel renders JSON as a collapsible
        // tree, text as <pre>, markdown via react-markdown, auto-collapsing long values. No MCP tool —
        // it dispatches inside `flows.run` like the data-pack nodes.
        NodeDescriptor::new("debug", NodeKind::Sink, "")
            .with_title("Debug")
            .with_category("Observability")
            .with_icon("bug")
            .with_ports(vec!["payload".into()], vec![])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "label": {"type": "string", "title": "Label", "description": "Shown in the debug panel to attribute this node's messages (defaults to the node id)."},
                        "format": {"type": "string", "enum": ["auto", "json", "text", "markdown"], "default": "auto",
                            "title": "Content type",
                            "description": "How the panel renders the payload. `auto` sniffs at publish time: JSON object/array → json; a string with markdown markers → markdown; else text."},
                        "collapse_bytes": {"type": "integer", "default": DEFAULT_COLLAPSE_BYTES, "minimum": 0,
                            "title": "Collapse threshold (bytes)",
                            "description": "Values larger than this render collapsed with a 'show more'. 0 = never collapse. The full value is always on the wire."},
                        "rate_limit": {"type": "integer", "default": DEFAULT_RATE_LIMIT, "minimum": 0,
                            "title": "Max messages/sec",
                            "description": "Publish governor; 0 = use the node default. Breach drops with a sentinel so the panel shows 'N dropped' rather than lagging."}
                    }
                }),
            ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_is_a_sink_with_one_payload_in_and_no_out() {
        let d = observability_descriptors();
        let dbg = d.iter().find(|x| x.r#type == "debug").unwrap();
        assert_eq!(dbg.kind, NodeKind::Sink);
        assert_eq!(dbg.inputs, vec!["payload".to_string()]);
        assert!(dbg.outputs.is_empty(), "debug is a terminal observer");
        assert_eq!(dbg.category, "Observability");
    }

    #[test]
    fn debug_config_compiles_and_carries_the_defaults() {
        let d = observability_descriptors();
        let dbg = d.iter().find(|x| x.r#type == "debug").unwrap();
        crate::config_schema::compile_schema(&dbg.config)
            .expect("debug config is valid JSON-Schema 2020-12");
        let collapse = dbg.config["properties"]["collapse_bytes"]["default"]
            .as_u64()
            .unwrap();
        assert_eq!(collapse, DEFAULT_COLLAPSE_BYTES);
        let rate = dbg.config["properties"]["rate_limit"]["default"]
            .as_u64()
            .unwrap();
        assert_eq!(rate, DEFAULT_RATE_LIMIT);
    }
}
