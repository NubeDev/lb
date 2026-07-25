//! The `store.query` `tools.catalog` descriptor (read-only SQL surface, channels-command-palette).
//! Declared in code next to the verb (FILE-LAYOUT); collected by `tools::host_descriptors`. Carries
//! a real JSON-Schema input so a model (or the palette) can FORM the call — the name-only row left
//! the caller guessing the arg names (`sql`, `vars`).
//!
//! The schema mirrors the arguments `call_store_query_tool` (`tool.rs`) actually reads — a required
//! `sql` string and an optional `vars` object — pulled straight from the JSON `input`, so the
//! property names are the wire names verbatim. `store.schema` takes no args, so it stays name-only.

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The canonical input schema for `store.query` — `{ sql, vars? }`. `sql` is a single read-only
/// SELECT/INFO/SHOW (parse-allowlisted host-side); `vars` binds `$`-params in the statement.
pub(crate) fn query_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "sql": {
                "type": "string",
                "x-lb": { "widget": "sql", "description": "a single read-only SELECT/INFO/SHOW" }
            },
            "vars": {
                "type": "object",
                "x-lb": { "description": "optional $-bound query parameters" }
            }
        },
        "required": ["sql"]
    })
}

/// The `store.query` descriptor. Qualified name (the catalog does not re-prefix host-native verbs).
pub fn query_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "store.query".to_string(),
        title: "a bounded, workspace-walled read-only SELECT over the embedded store".to_string(),
        group: "store".to_string(),
        input_schema: Some(query_schema()),
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `store.query`'s schema names the exact args the bridge reads (`sql` required, `vars`
    /// optional) — the wire names verbatim, with the `sql` mini-editor widget hint.
    #[test]
    fn query_schema_matches_the_bridge_args() {
        let s = query_schema();
        assert_eq!(s["type"], "object");
        assert_eq!(s["properties"]["sql"]["type"], "string");
        assert_eq!(s["properties"]["sql"]["x-lb"]["widget"], "sql");
        assert_eq!(s["properties"]["vars"]["type"], "object");
        let required = s["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert!(required.contains(&json!("sql")));
    }
}
