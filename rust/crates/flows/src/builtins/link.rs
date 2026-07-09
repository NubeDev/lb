//! The **link** built-in pair — Node-RED's wireless link nodes (flow-input-ports-scope "Intent"
//! step 5). `link-out {target}` and `link-in {name}` give the OR-funnel pattern an editor affordance
//! that does **not** want a physical wire: a `link-out` names a target; every `link-out` naming `T`
//! feeds the one `link-in {name: "T"}`. The pair is the canonical `any`-policy collector — "many
//! sources → one handler, fire per message" — and the load-bearing topology the scope's
//! propagate-one-hop-past-the-funnel test exercises.
//!
//! The "wireless" promise is **editor sugar only**. The engine sees ordinary port-targeted wires:
//! [`crate::link::resolve_links`] rewrites each `link-out {target:T}`'s upstream(s) onto the matching
//! `link-in {name:T}` (dropping the `link-out` from the run graph) at run load, so the `any`-funnel
//! runtime + the `fctx` seam from flow-input-ports-scope Slice 2 carry the multiplicity. Save-time
//! [`crate::link::validate_links`] catches a `link-out` targeting a missing `link-in` (and a `link-in`
//! with no sources at all). Same workspace wall, same run, no new cap (rule 7), no core branch on the
//! ids beyond these built-in descriptors (rule 10).
//!
//! - `link-out` is a `sink`-kind naming node (one `payload` in, no out): its input defaults to `any`
//!   (so a multi-source link-out saves green), and **nothing may wire from it** (validate_links
//!   rejects a node that lists a `link-out` in its `needs` — its only job is to name a target).
//! - `link-in` is a transform with one `any` primary input + one `payload` output, named by
//!   `config.name`. It fires **once per resolved upstream** (Node-RED OR), each firing carrying that
//!   one upstream's envelope; a downstream `W` then settles once per `link-in` firing (the `fctx`
//!   propagates past the funnel).

use serde_json::json;

use crate::descriptor::{InputPort, JoinPolicy, NodeDescriptor, NodeKind};

/// The v1 link pack: `link-out` (the wireless sender) + `link-in` (the `any`-funnel collector). They
/// render through the same palette path as every built-in; the canvas shows them as their descriptors
/// (flow-input-ports-scope Slice 4). Resolution + validation live in [`crate::link`] (pure graph math).
pub fn link_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // The wireless SENDER. One `payload` in, no out (it names a target and is then dropped from
        // the run graph by `resolve_links`). `sink` kind so its input defaults to `any` — a multi-
        // source link-out forwards every source to its target without a save-time join lint. Nothing
        // may wire from a link-out (validate_links); its output is the wireless name, not a port.
        NodeDescriptor::new("link-out", NodeKind::Sink, "")
            .with_title("Link out")
            .with_category("Links")
            .with_icon("unlink")
            .with_ports(vec!["payload".into()], vec![])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["target"],
                    "additionalProperties": false,
                    "properties": {
                        "target": {"type": "string", "title": "Target name", "description": "The `link-in` name this node forwards to. Every `link-out` naming the same target feeds the one `link-in {name: <target>}` (resolved at run load into ordinary wires)."}
                    }
                }),
            ),
        // The wireless COLLECTOR. One `any` primary input (it fires once per resolved upstream — the
        // Node-RED OR funnel) + one `payload` output. Named by `config.name`; the resolved virtual
        // edges land on its primary port and the `fctx` seam propagates the multiplicity downstream.
        NodeDescriptor::new("link-in", NodeKind::Transform, "")
            .with_title("Link in")
            .with_category("Links")
            .with_icon("link")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_input_ports(vec![InputPort {
                name: "payload".into(),
                join: JoinPolicy::Any,
            }])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["name"],
                    "additionalProperties": false,
                    "properties": {
                        "name": {"type": "string", "title": "Name", "description": "The name every `link-out {target: <name>}` resolves onto. A `link-in` fires once per resolved upstream (an `any` funnel)."}
                    }
                }),
            ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_out_is_a_naming_sink_with_one_payload_in() {
        let d = link_descriptors();
        let lo = d.iter().find(|x| x.r#type == "link-out").unwrap();
        assert_eq!(lo.kind, NodeKind::Sink);
        assert_eq!(lo.inputs, vec!["payload".to_string()]);
        assert!(lo.outputs.is_empty(), "link-out has no output (wireless)");
        assert_eq!(lo.category, "Links");
        // sink ⇒ its input defaults to `any` (a multi-source link-out saves green).
        assert_eq!(lo.join_of(None), JoinPolicy::Any);
    }

    #[test]
    fn link_in_is_an_any_funnel_transform() {
        let d = link_descriptors();
        let li = d.iter().find(|x| x.r#type == "link-in").unwrap();
        assert_eq!(li.kind, NodeKind::Transform);
        assert_eq!(li.inputs, vec!["payload".to_string()]);
        assert_eq!(li.outputs, vec!["payload".to_string()]);
        // The overriding input_ports table makes link-in's primary an `any` funnel (Node-RED OR).
        assert_eq!(li.join_of(None), JoinPolicy::Any);
        assert_eq!(li.join_of(Some("payload")), JoinPolicy::Any);
        assert_eq!(li.category, "Links");
    }

    #[test]
    fn link_configs_compile() {
        for desc in link_descriptors() {
            crate::config_schema::compile_schema(&desc.config)
                .unwrap_or_else(|e| panic!("link {} config does not compile: {e}", desc.r#type));
        }
    }
}
