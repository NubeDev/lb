//! The **Data** category descriptors (data-nodes scope, Tier A): `change` / `select` / `merge` /
//! `map` / `flatten` / `sort` / `range` / `aggregate` / `template`. Pure, host-resolved, one input
//! `payload` → one output `payload` — the declarative reshape/scale/reduce work that used to need a
//! hand-written `rhai` cage. The execution logic is the pure [`crate::ops::data`] / [`crate::ops::template`]
//! functions (unit-tested in-crate); these descriptors give the editor the palette + config form.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// A `change`-style ordered-op list schema, shared by `change` and `map` (Risk 5 — one grammar).
fn ops_schema() -> serde_json::Value {
    json!({
        "type": "array",
        "description": "ordered ops applied in sequence to the payload",
        "items": {
            "type": "object",
            "required": ["op"],
            "additionalProperties": true,
            "properties": {
                "op": {"type": "string", "enum": ["set", "delete", "move", "copy"]},
                "path": {"type": "string", "description": "dot-path target (set/delete)"},
                "value": {"description": "the value to set (op=set)"},
                "from": {"type": "string", "description": "source dot-path (move/copy)"},
                "to": {"type": "string", "description": "destination dot-path (move/copy)"}
            }
        }
    })
}

fn transform(ty: &str, title: &str, icon: &str, config: serde_json::Value) -> NodeDescriptor {
    NodeDescriptor::new(ty, NodeKind::Transform, "")
        .with_title(title)
        .with_category("Data")
        .with_icon(icon)
        .with_ports(vec!["payload".into()], vec!["payload".into()])
        .with_config(1, config)
}

/// The nine Data-category descriptors.
pub fn data_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // Declarative reshape: ordered set/move/copy/delete ops on the payload (no `rhai` for "rename
        // a field").
        transform(
            "change",
            "Change (reshape)",
            "pencil",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {"ops": ops_schema()}
            }),
        ),
        // Project the payload down to a chosen set of field paths → a new object.
        transform(
            "select",
            "Select (keep keys)",
            "filter",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "paths": {"type": "array", "items": {"type": "string"}, "description": "dot-paths to keep"}
                }
            }),
        ),
        // Deep-merge an array of objects into one (last-writer-wins on scalar conflict).
        transform(
            "merge",
            "Merge (deep)",
            "combine",
            json!({"type": "object", "additionalProperties": false, "properties": {}}),
        ),
        // Apply a `change`-style op set over every element of an array payload.
        transform(
            "map",
            "Map (per element)",
            "list",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {"ops": ops_schema()}
            }),
        ),
        // Flatten a nested array (configurable depth) or dot-key a nested object.
        transform(
            "flatten",
            "Flatten",
            "unfold-vertical",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "depth": {"type": "integer", "description": "array levels to flatten; <=0/absent = fully deep"}
                }
            }),
        ),
        // Sort an array by field path + asc/desc, numeric or lexical.
        transform(
            "sort",
            "Sort",
            "arrow-down-up",
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "path": {"type": "string", "description": "field dot-path to sort by (absent = whole element)"},
                    "order": {"type": "string", "enum": ["asc", "desc"], "default": "asc"},
                    "numeric": {"type": "boolean", "default": false, "description": "numeric compare (else lexical)"}
                }
            }),
        ),
        // Linearly scale a numeric payload from an input range to an output range (sensor→eng-unit).
        transform(
            "range",
            "Range (scale)",
            "sliders-horizontal",
            json!({
                "type": "object",
                "required": ["inMin", "inMax", "outMin", "outMax"],
                "additionalProperties": false,
                "properties": {
                    "inMin": {"type": "number"},
                    "inMax": {"type": "number"},
                    "outMin": {"type": "number"},
                    "outMax": {"type": "number"},
                    "clamp": {"type": "boolean", "default": false, "description": "clamp the result to the output range"}
                }
            }),
        ),
        // Reduce an array payload to a scalar: sum/min/max/mean/count/concat.
        transform(
            "aggregate",
            "Aggregate (reduce)",
            "sigma",
            json!({
                "type": "object",
                "required": ["op"],
                "additionalProperties": false,
                "properties": {
                    "op": {"type": "string", "enum": ["sum", "min", "max", "mean", "count", "concat"]},
                    "path": {"type": "string", "description": "field dot-path per element (absent = the element)"},
                    "sep": {"type": "string", "default": "", "description": "separator (op=concat)"}
                }
            }),
        ),
        // Render a mustache-lite text template from payload fields → a string.
        transform(
            "template",
            "Template (text)",
            "file-text",
            json!({
                "type": "object",
                "required": ["template"],
                "additionalProperties": false,
                "properties": {
                    "template": {"type": "string", "description": "text with {{dot.path}} holes"}
                }
            }),
        ),
    ]
}
