//! The built-in node descriptors (node-descriptor-scope "The built-in descriptors"). They ship
//! **with the host** but expose the **identical** [`NodeDescriptor`] shape so the editor renders them
//! through the same palette path as extension nodes — one registry, one renderer, no "is this
//! native?" branch. They map onto the spine's node model (`flows-scope.md` "The node model").
//!
//! The set is split by category (FILE-LAYOUT — data-nodes Open Q5, resolved: a file per category so
//! `builtins.rs` stays under the 400-line rule as it grows from 8 to 28 descriptors):
//! - [`core`] — the spine built-ins (`trigger`/`flipflop`/`tool`/`rhai`/`rule`/`count`/`json`/`counter`/`subflow`/`approval`/`sink`).
//! - [`data`] — nine reshape/scale/reduce nodes (`change`/`select`/`merge`/`map`/`flatten`/`sort`/`range`/`aggregate`/`template`).
//! - [`parse`] — four text↔structure nodes (`csv`/`xml`/`yaml`/`base64`), malformed input FAILS the node.
//! - [`sequence`] — `split`/`join` (array-carry + the `parts` contract) + `batch` (durable grouping).
//! - [`function`] — `filter` (RBE), `unique` (dedupe), `switch` (routing), `delay` (durable park).
//! - [`observability`] — `debug` (Node-RED's debug node: a motion-only sink the debug panel tails).
//!
//! Every built-in speaks the **message envelope** (flow-message-envelope-scope D6): input port
//! `payload` (+ `topic` carried alongside), output `payload` (+ any field it sets, e.g. the sequence
//! `parts` metadata, which rides as a carried envelope field rather than a wired port).

pub mod core;
pub mod data;
pub mod function;
pub mod observability;
pub mod parse;
pub mod sequence;

use crate::descriptor::NodeDescriptor;

/// The full built-in registry: the spine nodes ∪ the data/JSON pack ∪ the observability nodes, in
/// the one shared shape. The merged `flows.nodes` verb unions this with each installed extension's
/// `[[node]]` descriptors.
pub fn builtin_descriptors() -> Vec<NodeDescriptor> {
    let mut out = core::core_descriptors();
    out.extend(data::data_descriptors());
    out.extend(parse::parse_descriptors());
    out.extend(sequence::sequence_descriptors());
    out.extend(function::function_descriptors());
    out.extend(observability::observability_descriptors());
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::NodeKind;

    /// The full expected type set (order = category order). Guards against an accidental drop/rename.
    const EXPECTED: &[&str] = &[
        // core (12)
        "trigger",
        "flipflop",
        "webhook",
        "tool",
        "rhai",
        "rule",
        "count",
        "json",
        "counter",
        "subflow",
        "approval",
        "sink",
        // data (9)
        "change",
        "select",
        "merge",
        "map",
        "flatten",
        "sort",
        "range",
        "aggregate",
        "template",
        // parse (4)
        "csv",
        "xml",
        "yaml",
        "base64",
        // sequence (3)
        "split",
        "join",
        "batch",
        // function (4)
        "filter",
        "unique",
        "switch",
        "delay",
        // observability (1)
        "debug",
    ];

    #[test]
    fn builtins_in_one_shape() {
        let d = builtin_descriptors();
        let types: Vec<&str> = d.iter().map(|x| x.r#type.as_str()).collect();
        assert_eq!(types, EXPECTED);
        assert_eq!(
            d.len(),
            33,
            "12 spine + 20 data/JSON pack + 1 observability"
        );
        // Every built-in carries a compilable config schema (the load-time contract this test owns).
        for desc in &d {
            crate::config_schema::compile_schema(&desc.config)
                .unwrap_or_else(|e| panic!("builtin {} config does not compile: {e}", desc.r#type));
        }
        // Every built-in carries a palette icon.
        for desc in &d {
            assert!(desc.icon.is_some(), "builtin {} has no icon", desc.r#type);
        }
        // No duplicate types.
        let mut seen = std::collections::HashSet::new();
        for desc in &d {
            assert!(
                seen.insert(desc.r#type.clone()),
                "duplicate builtin {}",
                desc.r#type
            );
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
    fn flipflop_is_a_no_input_trigger() {
        let d = builtin_descriptors();
        let ff = d.iter().find(|x| x.r#type == "flipflop").unwrap();
        assert_eq!(ff.kind, NodeKind::Trigger);
        assert!(ff.inputs.is_empty(), "flipflop is a source: no input port");
        assert_eq!(ff.outputs, vec!["payload".to_string(), "topic".to_string()]);
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
        // (`parts` rides as a carried envelope field, not a wired port, so it never appears here.)
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

    #[test]
    fn data_pack_nodes_are_envelope_transforms() {
        // Every one of the twenty data/JSON-pack nodes is a payload→payload transform (the drop-in
        // mould). They follow the 12 spine nodes (trigger…sink), so the pack is `EXPECTED[12..32]`.
        // `EXPECTED[32..]` is the observability pack (`debug`, a sink — NOT a transform), excluded here.
        let d = builtin_descriptors();
        for ty in &EXPECTED[12..32] {
            let desc = d.iter().find(|x| &x.r#type == ty).unwrap();
            assert_eq!(desc.kind, NodeKind::Transform, "{ty} should be a transform");
            assert_eq!(desc.inputs, vec!["payload".to_string()], "{ty} input");
            assert_eq!(desc.outputs, vec!["payload".to_string()], "{ty} output");
        }
    }
}
