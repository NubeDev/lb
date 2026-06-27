//! The MCP bridge for tag verbs — host-native tools under the one MCP contract (README §6.5). UI,
//! agents, and extensions reach `tags.add` / `tags.remove` / `tags.of` / `tags.find` the SAME way
//! they reach any wasm tool. Each verb authorizes first (the deny gate); denials are opaque
//! (`ToolError::Denied`). There is NO event-registration verb (host-internal only, tags scope).
//!
//! `tags.find` takes one polymorphic query object: `{ "facets": [ {"key": "...", "value": <any>?} ]}`
//! — a facet with a `value` is exact, without is key-only; all facets intersect (the resolved lean:
//! one object dispatched to the right mode).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use lb_tags::{Facet, Provenance, Source, Tag};
use serde_json::{json, Value};

use super::{tags_add, tags_find, tags_of, tags_remove, TagsError};

/// Dispatch a `tags.<verb>` MCP call. `input` is the verb's JSON arguments; the return is JSON.
pub async fn call_tags_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "tags.add" => {
            let entity = str_arg(input, "entity")?;
            let tag = Tag::new(str_arg(input, "key")?, arg(input, "value")?.clone());
            let prov = provenance(input, principal)?;
            tags_add(store, principal, ws, entity, &tag, &prov)
                .await
                .map_err(tags_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "tags.remove" => {
            let entity = str_arg(input, "entity")?;
            let key = str_arg(input, "key")?;
            let value = input.get("value");
            tags_remove(store, principal, ws, entity, key, value)
                .await
                .map_err(tags_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "tags.of" => {
            let entity = str_arg(input, "entity")?;
            let applied = tags_of(store, principal, ws, entity)
                .await
                .map_err(tags_to_tool)?;
            Ok(json!({ "tags": applied }))
        }
        "tags.find" => {
            let facets = facets(input)?;
            let hits = tags_find(store, principal, ws, &facets)
                .await
                .map_err(tags_to_tool)?;
            Ok(json!({ "entities": hits }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Build the provenance from the input, defaulting `at`/`confidence` and the calling principal as
/// `by`. `source` defaults to `human` (the caller is a person/agent unless it says otherwise).
fn provenance(input: &Value, principal: &Principal) -> Result<Provenance, ToolError> {
    let at = input.get("at").and_then(|v| v.as_u64()).unwrap_or(0);
    let source = match input.get("source").and_then(|v| v.as_str()) {
        Some("inferred") => Source::Inferred,
        Some("producer") => Source::Producer,
        Some("system") => Source::System,
        _ => Source::Human,
    };
    let mut p = Provenance::new(at, principal.sub().to_string(), source);
    if let Some(c) = input.get("confidence").and_then(|v| v.as_f64()) {
        p.confidence = c;
    }
    p.expires = input.get("expires").and_then(|v| v.as_u64());
    Ok(p)
}

/// Parse the `facets` array of the find query into `Facet`s (value present → exact; absent → key-only).
fn facets(input: &Value) -> Result<Vec<Facet>, ToolError> {
    let arr = input
        .get("facets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::BadInput("missing facets array".into()))?;
    arr.iter()
        .map(|f| {
            let key = f
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("facet missing key".into()))?;
            Ok(match f.get("value") {
                Some(v) => Facet::exact(key, v.clone()),
                None => Facet::key_only(key),
            })
        })
        .collect()
}

fn tags_to_tool(e: TagsError) -> ToolError {
    match e {
        TagsError::Denied => ToolError::Denied,
        TagsError::CapExceeded(c) => ToolError::BadInput(format!("tag-node cap exceeded ({c})")),
        TagsError::BadInput(m) => ToolError::BadInput(m),
        TagsError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}
