//! The **host-native tool descriptors** + JSON-Schema arg validation (channels-command-palette
//! scope). Each host-native verb the palette drives declares its [`ToolDescriptor`] — `{ name,
//! title, group, input_schema }` where `input_schema` is a standard JSON Schema (`type:"object"`,
//! `properties`, `required`) with per-property `x-lb-entity` / `x-lb-widget` vendor hints. The
//! descriptors for host-native verbs live in code next to the verb (one `descriptor()` per verb
//! file, FILE-LAYOUT); this file is the collector `tools.catalog` walks, plus the defense-in-depth
//! arg validator the dispatcher runs before delegating to a handler.
//!
//! Why a JSON Schema and not a bespoke form model: standard JSON Schema composes with off-the-shelf
//! validators/tooling, and the two vendor hints are all the UI needs to drive a guided rail without
//! reinventing a form engine (scope "Arg-schema expressiveness"). `input_schema = None` is valid —
//! the palette degrades to a single free-text arg and the validator skips.

use lb_mcp::{ToolDescriptor, ToolError};
use serde_json::{json, Value};

/// The host-native descriptors the catalog serves alongside the extension half (read from the
/// registry). Today the palette's first tenant is `federation.query`; as more verbs gain a guided
/// rail their `descriptor()` is added here. Each carries its qualified name (the catalog does NOT
/// re-prefix host-native verbs) and a title/group for the menu.
pub(crate) fn host_descriptors() -> Vec<ToolDescriptor> {
    let mut out: Vec<ToolDescriptor> = vec![
        crate::federation::query_descriptor(),
        // The discovery twin (datasources-ux scope): a real `{source, table?}` schema so a model
        // (or the palette) can form the call — without it the agent probes `information_schema`
        // SQL through federation.query and hits the steering rejection instead of the answer.
        crate::federation::schema_descriptor(),
        // The AI-context snapshot (datasource-samples scope): one call returning tables + columns
        // + foreign keys + LIMIT-n rows, so a model can write correct SQL without probing.
        crate::federation::sample_descriptor(),
        // The write plane (schema-designer scope): bounded INSERT/UPSERT + DDL migrate + durable
        // export. Each gates on its own cap; `federation.migrate` is admin + dry-run-default.
        crate::federation::write_descriptor(),
        crate::federation::migrate_descriptor(),
        crate::federation::export_descriptor(),
        crate::query::save_descriptor(),
        crate::query::run_descriptor(),
        crate::query::compile_descriptor(),
        // The in-channel agent as a first-class palette command. Named `agent.invoke` on purpose: the
        // catalog gates each tool on `authorize_tool(principal, ws, <name>)`, so the run's existing
        // `mcp:agent.invoke:call` gate ALSO decides this command's visibility — no new cap, no `if` in
        // the catalog (see `agent::descriptor`). The palette routes it to `postAgent`, not a raw call.
        crate::agent::invoke_descriptor(),
        // The reminder palette commands. Named `reminder.<verb>` on purpose: the catalog gates each on
        // `authorize_tool(principal, ws, <name>)`, so each verb's OWN `mcp:reminder.<verb>:call` gate
        // decides its visibility — no new cap, no `if` in the catalog (see `reminder::descriptor`).
        crate::reminder::create_descriptor(),
        crate::reminder::list_descriptor(),
        crate::reminder::fire_descriptor(),
        // The widget palette read (widget-catalog scope). Named `dashboard.catalog` on purpose: the
        // catalog gates each tool on `authorize_tool(principal, ws, <name>)`, so the verb's own
        // `mcp:dashboard.catalog:call` gate decides its visibility — no new cap, no `if` in the catalog.
        crate::dashboard::catalog_descriptor(),
        // The pin-to-dashboard write (widget-platform scope, Slice B). Named `dashboard.pin` on purpose:
        // the catalog gates each tool on `authorize_tool(principal, ws, <name>)`, so the verb's own
        // `mcp:dashboard.pin:call` gate decides its visibility — no new cap, no `if` in the catalog. The
        // verb mints a persisted cell from any `x-lb-render` envelope (generic over the tool id, rule 10).
        crate::dashboard::pin_descriptor(),
        // The dashboard upsert (dashboard scope). A real schema so a model can form the call — the
        // name-only row left the live agent sending `cells` as a JSON-encoded string every turn
        // (see save.rs::save_descriptor).
        crate::dashboard::save_descriptor(),
        // The visibility write (dashboard scope) — schema'd so a model can form the call.
        crate::dashboard::share_descriptor(),
        // Grafana JSON import/export (viz import-export scope, Phase 4) — schema'd so a model can form
        // the two-phase import call. Gated by each verb's own cap in the catalog (no `if`).
        crate::dashboard::import_descriptor(),
        crate::dashboard::export_descriptor(),
        // The channel write (channel-widgets scope) — schema'd so a model can form the call; the
        // name-only row left the live agent guessing arg names ("missing arg: cid" × a whole run).
        crate::channel::post_descriptor(),
        // The channel register verb (collaboration scope) — makes a channel `channel.list`-visible
        // before the first post. Reuses the channel `pub` gate (no new cap), like `channel_create`.
        crate::channel::create_descriptor(),
        // The doc-extraction verb (doc-extraction scope). Named `docs.extract` on purpose: the
        // catalog gates each tool on `authorize_tool(principal, ws, <name>)`, so the verb's own
        // `mcp:docs.extract:call` gate decides its visibility — no new cap, no `if` in the catalog.
        crate::extract_descriptor(),
    ];
    out.extend(crate::host_tools::secret_descriptors());
    out
}

/// Validate a tool-call `input` against its declared JSON Schema `input_schema` (defense in depth
/// — the per-verb handler still does its own checks). A request failing validation is a clean
/// [`ToolError::BadInput`], never a panic. `None` schema → pass (the tool declares nothing, so any
/// object is accepted; the handler remains authoritative). Implements the small JSON-Schema subset
/// the palette's verbs use (`type: object`, `properties.<name>.type`, `required`); an unknown
/// schema facet is ignored (fail-open on structure, the handler is the real gate).
pub(crate) fn validate_args(schema: Option<&Value>, input: &Value) -> Result<(), ToolError> {
    let Some(schema) = schema else {
        return Ok(());
    };
    let obj = schema
        .as_object()
        .ok_or_else(|| ToolError::BadInput("input_schema must be a JSON object".to_string()))?;
    // `type: "object"` — the input must be a JSON object (the MCP call envelope).
    if matches!(obj.get("type").and_then(Value::as_str), Some("object")) && !input.is_object() {
        return Err(ToolError::BadInput("expected an object".to_string()));
    }
    // `required` — each listed property must be present and non-null. The miss names the arg's own
    // declared `x-lb.description` when there is one: this error feeds back into an agent loop, and a
    // bare arg name taught the live model nothing (13 × "missing arg: cid" in one run, 2026-07-06).
    if let Some(required) = obj.get("required").and_then(Value::as_array) {
        let input_obj = input
            .as_object()
            .ok_or_else(|| ToolError::BadInput("expected an object".to_string()))?;
        for key in required.iter().filter_map(Value::as_str) {
            match input_obj.get(key) {
                None | Some(Value::Null) => {
                    let hint = obj
                        .get("properties")
                        .and_then(|p| p.get(key))
                        .and_then(|p| p.get("x-lb"))
                        .and_then(|x| x.get("description"))
                        .and_then(Value::as_str)
                        .map(|d| format!(" — {d}"))
                        .unwrap_or_default();
                    return Err(ToolError::BadInput(format!(
                        "missing required arg: {key}{hint}"
                    )));
                }
                _ => {}
            }
        }
    }
    // `properties.<name>.type` — a shallow type check per present property.
    if let Some(props) = obj.get("properties").and_then(Value::as_object) {
        let input_obj = input
            .as_object()
            .ok_or_else(|| ToolError::BadInput("expected an object".to_string()))?;
        for (name, prop) in props {
            if let Some(value) = input_obj.get(name) {
                if let Some(want) = prop.get("type").and_then(Value::as_str) {
                    if !type_matches(want, value) {
                        return Err(ToolError::BadInput(format!("arg `{name}` must be {want}")));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Does `value` satisfy the JSON-Schema `type` keyword `want`?
fn type_matches(want: &str, value: &Value) -> bool {
    match want {
        "string" => value.is_string(),
        "number" | "integer" => value.is_number(),
        "boolean" => value.is_boolean(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "null" => value.is_null(),
        _ => true, // unknown type keyword → don't fail (the handler is the real gate)
    }
}

/// Build the canonical JSON Schema for `federation.query`'s input — `{source, sql, cache?}` —
/// shared by the host descriptor and mirrored by the UI type. `x-lb-entity: datasource` drives the
/// `@`-picker; `x-lb-widget: sql` selects the mini SQL editor.
///
/// `cache` is the optional, opt-in result-cache contract (federation-result-cache scope): a caller
/// that declares a freshness window may be served a repeat of an identical query from the
/// federation child's memory. Omitting it is today's behaviour bit for bit — the cache is never
/// assumed, because only the surface that knows its own refresh contract may accept staleness.
///
/// `ttl_s` is REQUIRED inside `cache`: an empty `cache: {}` is a schema error rather than a default,
/// because the child must never invent a freshness contract on a caller's behalf. (The schema sets
/// no `additionalProperties: false`, so before this field was parsed host-side a caller-sent `cache`
/// validated fine and was then silently dropped by the enumerated child-input build — which is worse
/// than a rejection. That is why the parse, the signature, the `json!`, and this schema ship
/// together or not at all.)
pub(crate) fn federation_query_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "source": { "type": "string", "x-lb": { "entity": "datasource" } },
            "sql": { "type": "string", "x-lb": { "widget": "sql" } },
            "cache": {
                "type": "object",
                "description": "Opt-in result caching. Serves an identical repeat of this query \
                                from the federation child's memory when it is younger than ttl_s. \
                                Worst-case staleness is ttl_s + the caller's refresh interval, so \
                                size ttl_s slightly below that interval if the tighter bound matters.",
                "properties": {
                    "ttl_s": {
                        "type": "number",
                        "description": "Freshness window in seconds. 0 disables caching for this call."
                    }
                },
                "required": ["ttl_s"]
            }
        },
        "required": ["source", "sql"]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn federation_query_schema_is_well_formed() {
        let s = federation_query_schema();
        assert_eq!(s["type"], "object");
        assert_eq!(s["properties"]["source"]["x-lb"]["entity"], "datasource");
        assert_eq!(s["properties"]["sql"]["x-lb"]["widget"], "sql");
        let required = s["required"].as_array().unwrap();
        assert!(required.contains(&json!("source")));
        assert!(required.contains(&json!("sql")));
    }

    #[test]
    fn none_schema_passes_anything() {
        validate_args(None, &json!({})).unwrap();
        validate_args(None, &json!("whatever")).unwrap();
    }

    #[test]
    fn missing_required_arg_is_bad_input() {
        let s = federation_query_schema();
        let err = validate_args(Some(&s), &json!({ "source": "warehouse" })).unwrap_err();
        assert!(matches!(err, ToolError::BadInput(m) if m.contains("sql")));
    }

    #[test]
    fn wrong_type_is_bad_input() {
        let s = federation_query_schema();
        let err = validate_args(Some(&s), &json!({ "source": 5, "sql": "SELECT 1" })).unwrap_err();
        assert!(matches!(err, ToolError::BadInput(m) if m.contains("source")));
    }

    // The channel.post schema exists (the name-only row burned a live run on guessed arg names) and
    // a missing required arg's error carries the arg's own x-lb description — the agent-loop feedback.
    #[test]
    fn channel_post_missing_cid_error_names_where_the_cid_comes_from() {
        let d = crate::channel::post_descriptor();
        let schema = d.input_schema.expect("channel.post declares a schema");
        let err = validate_args(Some(&schema), &json!({ "id": "m1", "body": "hi" })).unwrap_err();
        assert!(
            matches!(err, ToolError::BadInput(ref m) if m.contains("conversation channel")),
            "the miss must say where the cid comes from"
        );
    }

    #[test]
    fn valid_args_pass() {
        let s = federation_query_schema();
        validate_args(
            Some(&s),
            &json!({ "source": "warehouse", "sql": "SELECT 1" }),
        )
        .unwrap();
    }
}
