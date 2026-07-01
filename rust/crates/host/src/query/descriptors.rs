//! The `query.*` `tools.catalog` descriptors (query scope, channels-command-palette). Declared in
//! code next to the verbs (FILE-LAYOUT); collected by `tools::host_descriptors`. The editor-facing
//! verbs (`save`, `run`, `compile`) carry JSON-Schema inputs with `x-lb` hints (a `prql`/`sql` text
//! widget, a `datasource` entity picker for the target) — the same vocabulary `federation.query`'s
//! descriptor established.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The canonical input schema for `query.save` (the authoring verb).
pub(crate) fn save_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "name": { "type": "string" },
            "description": { "type": "string" },
            "lang": { "type": "string", "enum": ["prql", "raw"] },
            "text": { "type": "string", "x-lb": { "widget": "prql" } },
            "target": { "type": "string", "x-lb": { "entity": "datasource" } },
            "params": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["id", "lang", "text", "target"]
    })
}

/// The canonical input schema for `query.run`.
pub(crate) fn run_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "lang": { "type": "string", "enum": ["prql", "raw"] },
            "text": { "type": "string", "x-lb": { "widget": "prql" } },
            "target": { "type": "string", "x-lb": { "entity": "datasource" } },
            "vars": { "type": "object" }
        }
    })
}

/// The canonical input schema for `query.compile` (the dry-run preview).
pub(crate) fn compile_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "lang": { "type": "string", "enum": ["prql", "raw"] },
            "text": { "type": "string", "x-lb": { "widget": "prql" } },
            "target": { "type": "string", "x-lb": { "entity": "datasource" } }
        },
        "required": ["lang", "text", "target"]
    })
}

/// The `query.save` descriptor.
pub fn save_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "query.save".to_string(),
        title: "Save (upsert) a PRQL/raw query as an editable workspace record".to_string(),
        group: "query".to_string(),
        input_schema: Some(save_schema()),
        result: None,
    }
}

/// The `query.run` descriptor.
pub fn run_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "query.run".to_string(),
        title: "Run a saved/inline query against the platform store or a datasource".to_string(),
        group: "query".to_string(),
        input_schema: Some(run_schema()),
        result: None,
    }
}

/// The `query.compile` descriptor (dry-run, no data access).
pub fn compile_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "query.compile".to_string(),
        title: "Compile a PRQL/raw query to its target SQL without executing".to_string(),
        group: "query".to_string(),
        input_schema: Some(compile_schema()),
        result: None,
    }
}
