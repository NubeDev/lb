//! The **Sequence** category descriptors (data-nodes scope): `split` / `join` / `batch`. `split` and
//! `join` are the array-carry sequence pair (Decision 15 — one settle carries the array + a `parts`
//! descriptor, no per-message fan-out; the pure logic is [`crate::ops::sequence`]). `batch` is the
//! stateful grouping node (Tier B) — accumulate N incoming payloads into one array via the durable
//! bounded buffer (`flow_node_buffer`, force-release at the cap). All speak the envelope.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

fn seq(ty: &str, title: &str, icon: &str, config: serde_json::Value) -> NodeDescriptor {
    NodeDescriptor::new(ty, NodeKind::Transform, "")
        .with_title(title)
        .with_category("Sequence")
        .with_icon(icon)
        .with_ports(vec!["payload".into()], vec!["payload".into()])
        .with_config(1, config)
}

/// The three Sequence-category descriptors.
pub fn sequence_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // One array/object `payload` → the sequence envelope (`payload` = array + a carried `parts`
        // descriptor). `id` tags the sequence; the array rides one settle (array-carry, D15).
        seq(
            "split",
            "Split (to sequence)",
            "split",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "id": {"type": "string", "description": "sequence id stamped on parts (default \"seq\")"}
                }
            }),
        ),
        // Recombine a `split` sequence back into an array/object, keyed by the carried `parts`
        // (kind/keys). Stateless under array-carry — no config.
        seq(
            "join",
            "Join (from sequence)",
            "merge",
            json!({"type": "object", "additionalProperties": false, "properties": {}}),
        ),
        // Group N incoming payloads into one array `payload` (Tier B — a durable buffer between
        // firings; releases at `count`, force-releases at the buffer cap). Suppresses until release.
        seq(
            "batch",
            "Batch (group N)",
            "package",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "count": {"type": "integer", "default": 10, "minimum": 1, "description": "release every N payloads"}
                }
            }),
        ),
    ]
}
