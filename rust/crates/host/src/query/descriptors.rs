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
        emits_external: false,
        name: "query.save".to_string(),
        title: "Save (upsert) a PRQL/raw query as an editable workspace record".to_string(),
        group: "query".to_string(),
        input_schema: Some(save_schema()),
        result: None,
    }
}

/// The `query.run` **response render envelope** (`x-lb-render`), widget-platform Slice C. Same shape
/// as `reminder::list_render` and `federation::query_result_render`: the palette posts this verbatim
/// (interpolating collected args into `source.args`); the channel mounts it through the shipped
/// `WidgetView`; `dashboard.pin` mints a persisted `pin-query-run` cell from it (generic over the
/// tool id — rule 10). The verb returns `{columns, rows}` (run.rs:97), the columnar shape
/// `viz::frame::result_to_rows` zips into named row objects. The `source.tool` names the verb itself
/// (the re-runnable read); a pinned cell captures `source.args` at pin time (an `{id}` for a saved
/// query propagates edits — "this query, live"; or `{lang,text,target}` for an inline one-shot) and
/// re-runs it under the viewer's grant at render. `tools[]` is just the read (no row-control verbs).
pub(crate) fn run_result_render() -> Value {
    json!({
        "v": 2,
        "view": "table",
        "source": { "tool": "query.run", "args": {} },
        "tools": ["query.run"]
    })
}

/// The `query.run` descriptor.
pub fn run_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "query.run".to_string(),
        title: "Run a saved/inline query against the platform store or a datasource".to_string(),
        group: "query".to_string(),
        input_schema: Some(run_schema()),
        // The OUTPUT contract — widget-platform Slice C. The verb returns `{columns, rows}` (the
        // columnar shape `viz::frame::result_to_rows` is written for), so a `rich_result` table
        // `source`-d at `query.run` renders unchanged through the shipped `WidgetView`, and Slice B's
        // `dashboard.pin` mints a persisted `pin-query-run` cell from this envelope with ZERO
        // query-specific code in the pin path (generic over the tool id, rule 10).
        result: Some(run_result_render()),
    }
}

/// The `query.compile` descriptor (dry-run, no data access).
pub fn compile_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "query.compile".to_string(),
        title: "Compile a PRQL/raw query to its target SQL without executing".to_string(),
        group: "query".to_string(),
        input_schema: Some(compile_schema()),
        result: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `query.run`'s descriptor carries a `result = table` envelope (the OUTPUT contract) — same
    /// shape `reminder.list`'s render established. The source names the verb itself; the bridge
    /// re-runs it under the viewer's grant at render. Widget-platform Slice C.
    #[test]
    fn run_descriptor_carries_the_table_render() {
        let render = run_descriptor()
            .result
            .expect("query.run declares a result render");
        assert_eq!(render["v"], 2);
        assert_eq!(render["view"], "table");
        assert_eq!(render["source"]["tool"], "query.run");
        assert!(render["source"]["args"].is_object());
        // `tools[]` is just the read itself — a pure read has no row-control write verbs.
        let tools = render["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert!(tools.contains(&json!("query.run")));
    }

    /// `query.save` and `query.compile` do NOT declare a render — they are write/dry-run verbs whose
    /// answers are not pin-able widgets (the saved record / the compiled SQL). Named Slice C
    /// follow-ups, not silently dropped.
    #[test]
    fn save_and_compile_do_not_declare_a_render() {
        assert!(
            save_descriptor().result.is_none(),
            "query.save is a write verb"
        );
        assert!(
            compile_descriptor().result.is_none(),
            "query.compile is a dry-run; its SQL-text answer is a Slice C follow-up"
        );
    }
}
