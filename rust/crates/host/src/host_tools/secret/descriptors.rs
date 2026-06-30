//! JSON-Schema [`ToolDescriptor`]s for the `secret.*` verbs (channels-command-palette scope). The
//! palette reads these to render a guided argument rail for the secret surface; each schema is also
//! the defense-in-depth input check the dispatcher runs. Sensitive args (`value`) carry no special
//! widget — the UI is expected to render a masked field, but the schema only declares the shape.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The shared `path` property — every secret verb keys on the secret path.
fn path_prop() -> Value {
    json!({ "type": "string", "x-lb": { "hint": "secret path, e.g. ext/mqtt/broker-pw" } })
}

/// `secret.set {path, value, visibility?}`.
fn set_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": path_prop(),
            "value": { "type": "string", "x-lb": { "widget": "secret" } },
            "visibility": { "type": "string", "enum": ["private", "workspace"] }
        },
        "required": ["path", "value"]
    })
}

/// `secret.get {path}`.
fn get_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "path": path_prop() },
        "required": ["path"]
    })
}

/// `secret.set_visibility {path, visibility}`.
fn set_visibility_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "path": path_prop(),
            "visibility": { "type": "string", "enum": ["private", "workspace"] }
        },
        "required": ["path", "visibility"]
    })
}

/// `secret.delete {path}`.
fn delete_schema() -> Value {
    json!({
        "type": "object",
        "properties": { "path": path_prop() },
        "required": ["path"]
    })
}

/// The `secret.*` tool descriptors the command-palette catalog serves. `secret.list` takes no args,
/// so it has no schema (the palette renders it arg-free).
pub fn secret_descriptors() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "secret.set".to_string(),
            title: "Store (create/overwrite) a secret".to_string(),
            group: "secret".to_string(),
            input_schema: Some(set_schema()),
        },
        ToolDescriptor {
            name: "secret.get".to_string(),
            title: "Read a secret value (three-gate: owner for private, any member for workspace)"
                .to_string(),
            group: "secret".to_string(),
            input_schema: Some(get_schema()),
        },
        ToolDescriptor {
            name: "secret.set_visibility".to_string(),
            title: "Toggle a secret's visibility (private | workspace) — owner only".to_string(),
            group: "secret".to_string(),
            input_schema: Some(set_visibility_schema()),
        },
        ToolDescriptor {
            name: "secret.delete".to_string(),
            title: "Delete a secret — owner only".to_string(),
            group: "secret".to_string(),
            input_schema: Some(delete_schema()),
        },
        ToolDescriptor {
            name: "secret.list".to_string(),
            title: "List secret metadata (path/owner/visibility) — never the values".to_string(),
            group: "secret".to_string(),
            input_schema: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::validate_args;

    #[test]
    fn set_schema_requires_path_and_value() {
        let s = set_schema();
        let err = validate_args(Some(&s), &json!({ "path": "ext/mqtt/broker-pw" })).unwrap_err();
        assert!(matches!(err, lb_mcp::ToolError::BadInput(m) if m.contains("value")));
        validate_args(Some(&s), &json!({ "path": "p", "value": "v" })).unwrap();
    }

    #[test]
    fn set_visibility_schema_requires_both() {
        let s = set_visibility_schema();
        let err = validate_args(Some(&s), &json!({ "path": "p" })).unwrap_err();
        assert!(matches!(err, lb_mcp::ToolError::BadInput(m) if m.contains("visibility")));
        validate_args(
            Some(&s),
            &json!({ "path": "p", "visibility": "workspace" }),
        )
        .unwrap();
    }

    #[test]
    fn get_and_delete_require_path() {
        for schema in [get_schema(), delete_schema()] {
            let err = validate_args(Some(&schema), &json!({})).unwrap_err();
            assert!(matches!(err, lb_mcp::ToolError::BadInput(m) if m.contains("path")));
        }
    }

    #[test]
    fn every_descriptor_is_well_formed() {
        for d in secret_descriptors() {
            assert!(!d.name.is_empty());
            assert!(!d.title.is_empty());
            assert_eq!(d.group, "secret");
        }
    }
}
