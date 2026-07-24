//! The **Platform** category descriptors (ext-store-nodes scope): `ext-list` / `ext-call` /
//! `store-read` / `store-write` / `store-delete`. First-class nodes over the platform's own MCP
//! surface — enumerate/call installed extensions, and CRUD rows on a picked store table — each
//! dispatching the EXISTING verbs (`ext.list`, `<ext>.<tool>`, `store.query`, `store.write`,
//! `store.delete`) under the caller's principal through the one `call_tool` chokepoint. No new verb,
//! no new capability, no per-extension anything (rule 10 — `ext`/`tool` are opaque strings the
//! author picked; core never names one).
//!
//! The `lb:*` `format` strings are **editor picker hints** in the `lb:datasource` mold
//! (schema-designer scope): `lb:extension` (dropdown from `ext.list`), `lb:ext-tool` (dropdown from
//! `tools.catalog`, scoped to the sibling `ext`), `lb:store-table` (all tables) /
//! `lb:store-table-writable` (non-system tables only — the reserved-table wall's picker half). An
//! editor that doesn't know a format degrades to a text input; host-side `jsonschema` validation
//! ignores unknown formats, so the schemas stay pure JSON-Schema 2020-12.

use serde_json::json;

use crate::descriptor::{NodeDescriptor, NodeKind};

/// The five Platform-category descriptors, all speaking the message envelope (D6).
pub fn platform_descriptors() -> Vec<NodeDescriptor> {
    vec![
        // Enumerate the workspace's installed extensions (the `ext.list` rows: ext, version, tier,
        // enabled, running, health) so a flow can branch on "is X running?" without an agent.
        NodeDescriptor::new("ext-list", NodeKind::Transform, "")
            .with_title("Extensions (list installed)")
            .with_category("Platform")
            .with_icon("blocks")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "running_only": {"type": "boolean", "default": false, "description": "emit only extensions whose live state is running"}
                    }
                }),
            ),
        // Call ANY extension tool, fully picker-driven: pick the extension, pick its tool (scoped to
        // that extension via `tools.catalog`), fill the args form rendered from the tool's own
        // `input_schema`. Dispatches `<ext>.<tool>` under the caller's caps (`caller ∩ install-grant`
        // narrowing applies as ever); the incoming `payload` deep-merges into `args` — the `tool`
        // node's exact rule.
        NodeDescriptor::new("ext-call", NodeKind::Transform, "")
            .with_title("Extension call")
            .with_category("Platform")
            .with_icon("plug")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["ext", "tool"],
                    "additionalProperties": false,
                    "properties": {
                        "ext": {"type": "string", "format": "lb:extension", "description": "the installed extension to call (ext.list)"},
                        "tool": {"type": "string", "format": "lb:ext-tool", "description": "the extension's tool (tools.catalog, scoped to the picked ext)"},
                        "args": {"type": "object", "default": {}, "description": "fixed args (rendered from the tool's input_schema); an object payload merges over these"},
                        "timeout_ms": {"type": "integer", "minimum": 1, "description": "wall-clock ceiling for this node's dispatch, in ms (settles err:\"timeout\" if exceeded)"}
                    }
                }),
            ),
        // Read rows from a picked store table by id or flat field=value filter. The host builds a
        // PARAMETERIZED SELECT and dispatches `store.query` — never string-spliced from user text.
        // Emits `{rows: [...]}` (data unwrapped from the store's `{data, rev}` envelope); a
        // single-`id` read emits `{row}`.
        NodeDescriptor::new("store-read", NodeKind::Transform, "")
            .with_title("Store read")
            .with_category("Platform")
            .with_icon("database")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["table"],
                    "additionalProperties": false,
                    "properties": {
                        "table": {"type": "string", "format": "lb:store-table", "description": "the store table to read (store.tables)"},
                        "id": {"type": "string", "description": "read one row by id (payload.id when omitted; emits {row})"},
                        "filter": {"type": "object", "description": "flat field=value equality filter over the row data"},
                        "limit": {"type": "integer", "default": 100, "minimum": 1, "maximum": 1000, "description": "max rows (default 100, hard max 1000)"},
                        "order_by": {"type": "string", "description": "row-data field to order by"},
                        "desc": {"type": "boolean", "default": false, "description": "descending order (with order_by)"}
                    }
                }),
            ),
        // Upsert `{table, id, value}` on a picked NON-SYSTEM table via `store.write` (the writable
        // picker excludes system tables; the reserved-table wall in the verb is the real guard).
        // `id` defaults to a generated ULID; `value` defaults to the incoming payload. Emits
        // `{table, id}` so a downstream node learns the key.
        NodeDescriptor::new("store-write", NodeKind::Transform, "")
            .with_title("Store write")
            .with_category("Platform")
            .with_icon("save")
            .with_ports(vec!["payload".into()], vec!["payload".into()])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["table"],
                    "additionalProperties": false,
                    "properties": {
                        "table": {"type": "string", "format": "lb:store-table-writable", "description": "the (non-system) store table to upsert into"},
                        "id": {"type": "string", "description": "the row id (payload.id when omitted; else a generated ULID)"},
                        "value": {"type": "object", "description": "the row value (the incoming payload when omitted)"}
                    }
                }),
            ),
        // Delete `{table, id}` on a picked non-system table via `store.delete`. A terminal sink —
        // no outputs; the verb is idempotent (deleting an absent row succeeds).
        NodeDescriptor::new("store-delete", NodeKind::Sink, "")
            .with_title("Store delete")
            .with_category("Platform")
            .with_icon("trash-2")
            .with_ports(vec!["payload".into()], vec![])
            .with_config(
                1,
                json!({
                    "type": "object",
                    "required": ["table"],
                    "additionalProperties": false,
                    "properties": {
                        "table": {"type": "string", "format": "lb:store-table-writable", "description": "the (non-system) store table to delete from"},
                        "id": {"type": "string", "description": "the row id to delete (payload.id when omitted)"}
                    }
                }),
            ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// store-delete is a terminal sink: one `payload` in, no outputs (the scope table).
    #[test]
    fn store_delete_is_a_sink_with_no_outputs() {
        let d = platform_descriptors();
        let del = d.iter().find(|x| x.r#type == "store-delete").unwrap();
        assert_eq!(del.kind, NodeKind::Sink);
        assert_eq!(del.inputs, vec!["payload".to_string()]);
        assert!(del.outputs.is_empty(), "store-delete is a terminal sink");
        assert_eq!(del.category, "Platform");
    }

    /// The picker `format` hints ride the config schemas exactly as scoped: `lb:extension` /
    /// `lb:ext-tool` on ext-call, `lb:store-table` on the read, `lb:store-table-writable` on the
    /// two mutating nodes (the writable variant is the picker half of the reserved-table wall).
    #[test]
    fn configs_carry_the_lb_picker_formats() {
        let d = platform_descriptors();
        let fmt = |ty: &str, field: &str| -> String {
            let desc = d.iter().find(|x| x.r#type == ty).unwrap();
            desc.config["properties"][field]["format"]
                .as_str()
                .unwrap_or_else(|| panic!("{ty}.{field} has no format"))
                .to_string()
        };
        assert_eq!(fmt("ext-call", "ext"), "lb:extension");
        assert_eq!(fmt("ext-call", "tool"), "lb:ext-tool");
        assert_eq!(fmt("store-read", "table"), "lb:store-table");
        assert_eq!(fmt("store-write", "table"), "lb:store-table-writable");
        assert_eq!(fmt("store-delete", "table"), "lb:store-table-writable");
    }

    /// The required fields + the read bounds match the scope table (limit default 100, max 1000).
    #[test]
    fn required_fields_and_read_bounds_match_the_scope() {
        let d = platform_descriptors();
        let req = |ty: &str| -> Vec<String> {
            let desc = d.iter().find(|x| x.r#type == ty).unwrap();
            desc.config["required"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .map(|v| v.as_str().unwrap().to_string())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        };
        assert_eq!(req("ext-call"), vec!["ext", "tool"]);
        assert_eq!(req("store-read"), vec!["table"]);
        assert_eq!(req("store-write"), vec!["table"]);
        assert_eq!(req("store-delete"), vec!["table"]);
        assert_eq!(req("ext-list"), Vec::<String>::new());
        let read = d.iter().find(|x| x.r#type == "store-read").unwrap();
        let limit = &read.config["properties"]["limit"];
        assert_eq!(limit["default"], 100);
        assert_eq!(limit["maximum"], 1000);
    }

    /// Every platform config compiles as JSON-Schema 2020-12 (the load-time contract).
    #[test]
    fn platform_configs_compile() {
        for desc in platform_descriptors() {
            crate::config_schema::compile_schema(&desc.config).unwrap_or_else(|e| {
                panic!("platform {} config does not compile: {e}", desc.r#type)
            });
        }
    }
}
