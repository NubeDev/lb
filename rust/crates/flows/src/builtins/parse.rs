//! The **Parse** category descriptors (data-nodes scope, Tier A): `csv` / `xml` / `yaml` / `base64`.
//! Two-way text↔structure converters at a flow's boundary (a webhook body, a file read, an MQTT text
//! message) — the same parse/stringify duality the built-in `json` node has, and the **same failure
//! contract**: a malformed parse FAILS the node (Node-RED parity — a bad body surfaces instead of
//! flowing a wrong shape). Logic is the pure [`crate::ops::parse`] functions; these are the palette
//! entries. The parse crates (`csv`/`quick-xml`/`serde_yaml`/`base64`) are in `key-stack.md`.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

fn parse_node(
    ty: &str,
    title: &str,
    icon: &str,
    modes: [&str; 2],
    default_mode: &str,
) -> NodeDescriptor {
    NodeDescriptor::new(ty, NodeKind::Transform, "")
        .with_title(title)
        .with_category("Parse")
        .with_icon(icon)
        .with_ports(vec!["payload".into()], vec!["payload".into()])
        .with_config(
            1,
            json!({
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "mode": {"type": "string", "enum": [modes[0], modes[1]], "default": default_mode},
                    "header": {"type": "boolean", "default": true, "description": "first row is the header (csv only)"},
                    "separator": {"type": "string", "default": ",", "description": "delimiter (csv only)"}
                }
            }),
        )
}

/// The four Parse-category descriptors.
pub fn parse_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // CSV text ↔ array-of-objects (header row configurable). Malformed → fails.
        parse_node(
            "csv",
            "CSV (parse / stringify)",
            "table",
            ["parse", "stringify"],
            "parse",
        ),
        // XML text ↔ structured value (element→object, @attr, #text convention). Malformed → fails.
        parse_node(
            "xml",
            "XML (parse / stringify)",
            "code-xml",
            ["parse", "stringify"],
            "parse",
        ),
        // YAML text ↔ structured value. Malformed → fails.
        parse_node(
            "yaml",
            "YAML (parse / stringify)",
            "file-code",
            ["parse", "stringify"],
            "parse",
        ),
        // payload ↔ base64 (the text/bytes boundary). Invalid base64 on decode → fails.
        parse_node(
            "base64",
            "Base64 (encode / decode)",
            "binary",
            ["encode", "decode"],
            "encode",
        ),
    ]
}
