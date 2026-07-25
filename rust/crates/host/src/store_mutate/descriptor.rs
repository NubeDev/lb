//! The `store.write` / `store.delete` `tools.catalog` descriptors (store-mutation surface,
//! channels-command-palette). Declared in code next to the verbs (FILE-LAYOUT); collected by
//! `tools::host_descriptors`. Each carries a real JSON-Schema input so a model (or the palette)
//! can FORM the call — without a schema these verbs list name-only and the caller guesses the arg
//! names, exactly the failure the query/channel descriptors were added to fix.
//!
//! The schemas mirror the arguments `call_store_mutate_tool` (`tool.rs`) actually reads — `table`,
//! `id`, and (for write) `value` — pulled straight from the JSON `input` object, not a serde
//! struct, so the property names are the wire names verbatim (no camelCase rename to trip on).

use lb_mcp::ToolDescriptor;
use serde_json::{json, Value};

/// The canonical input schema for `store.write` — `{ table, id, value }`, all required. `value` is
/// an arbitrary JSON record (no `type` constraint) stored under the `{ data: … }` envelope.
pub(crate) fn write_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "table": {
                "type": "string",
                "x-lb": { "description": "the store table to upsert into" }
            },
            "id": {
                "type": "string",
                "x-lb": { "description": "the record id within the table" }
            },
            "value": {
                "x-lb": { "description": "the JSON record body to store" }
            }
        },
        "required": ["table", "id", "value"]
    })
}

/// The canonical input schema for `store.delete` — `{ table, id }`, both required.
pub(crate) fn delete_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "table": {
                "type": "string",
                "x-lb": { "description": "the store table to delete from" }
            },
            "id": {
                "type": "string",
                "x-lb": { "description": "the record id to erase (idempotent)" }
            }
        },
        "required": ["table", "id"]
    })
}

/// The `store.write` descriptor. Qualified name (the catalog does not re-prefix host-native verbs).
pub fn write_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "store.write".to_string(),
        title: "write one record into the embedded store".to_string(),
        group: "store".to_string(),
        input_schema: Some(write_schema()),
        result: None,
    }
}

/// The `store.delete` descriptor.
pub fn delete_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "store.delete".to_string(),
        title: "delete one record from the embedded store".to_string(),
        group: "store".to_string(),
        input_schema: Some(delete_schema()),
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `store.write`'s schema names the exact args the bridge reads (`table`, `id`, `value`) and
    /// marks all three required — the wire names verbatim (no serde rename), so a caller formed from
    /// the catalog lands the call.
    #[test]
    fn write_schema_matches_the_bridge_args() {
        let s = write_schema();
        assert_eq!(s["type"], "object");
        assert_eq!(s["properties"]["table"]["type"], "string");
        assert_eq!(s["properties"]["id"]["type"], "string");
        assert!(s["properties"]["value"].is_object());
        let required = s["required"].as_array().unwrap();
        assert!(required.contains(&json!("table")));
        assert!(required.contains(&json!("id")));
        assert!(required.contains(&json!("value")));
    }

    /// `store.delete` takes only `{ table, id }`, both required — no `value`.
    #[test]
    fn delete_schema_matches_the_bridge_args() {
        let s = delete_schema();
        assert_eq!(s["properties"]["table"]["type"], "string");
        assert_eq!(s["properties"]["id"]["type"], "string");
        assert!(s["properties"]["value"].is_null());
        let required = s["required"].as_array().unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains(&json!("table")));
        assert!(required.contains(&json!("id")));
    }
}
