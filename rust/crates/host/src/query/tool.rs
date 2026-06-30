//! The `query.*` MCP bridge (query scope, README §6.5/§7: the saved-query surface is reached as MCP
//! tools under the one contract). The UI, the agent, a rule, and other extensions reach a saved query
//! the SAME way they reach any tool — a qualified call with JSON in/out. Each verb authorizes first
//! (and `query.run` adds the no-widening target cap); denials are opaque (`ToolError::Denied`).

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::{query_compile, query_delete, query_get, query_list, query_run, query_save, RunSource};
use crate::boot::Node;

/// Dispatch a `query.*` MCP call. `input` is the verb's JSON args; the return is the verb's JSON
/// result. The MCP gate runs inside each service verb first (opaque `Denied`); `query.run` adds the
/// no-widening target cap inside its service.
pub async fn call_query_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
    match qualified_tool {
        "query.save" => {
            let id = str_arg(input, "id")?;
            let name = input.get("name").and_then(|v| v.as_str()).unwrap_or(id);
            let description = input.get("description").and_then(|v| v.as_str());
            let lang = str_arg(input, "lang")?;
            let text = str_arg(input, "text")?;
            let target = str_arg(input, "target")?;
            let params = str_array(input, "params");
            let saved = query_save(
                &node.store,
                principal,
                ws,
                id,
                name,
                description,
                lang,
                text,
                target,
                params,
                ts,
            )
            .await?;
            Ok(json!({ "id": saved }))
        }
        "query.get" => {
            let id = str_arg(input, "id")?;
            let q = query_get(&node.store, principal, ws, id).await?;
            Ok(serde_json::to_value(q).unwrap_or(Value::Null))
        }
        "query.list" => {
            let items = query_list(&node.store, principal, ws).await?;
            Ok(json!({ "queries": items }))
        }
        "query.delete" => {
            let id = str_arg(input, "id")?;
            query_delete(&node.store, principal, ws, id, ts).await?;
            Ok(json!({ "ok": true }))
        }
        "query.compile" => {
            let lang = str_arg(input, "lang")?;
            let text = str_arg(input, "text")?;
            let target = str_arg(input, "target")?;
            query_compile(node, principal, ws, lang, text, target)
                .await
                .map_err(ToolError::from)
        }
        "query.run" => {
            let src = run_source(input)?;
            let vars = vars_arg(input);
            query_run(node, principal, ws, src, vars, ts)
                .await
                .map_err(ToolError::from)
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Build the [`RunSource`] — `{id}` for a saved query, or inline `{lang, text, target}` for a one-shot.
fn run_source(input: &Value) -> Result<RunSource, ToolError> {
    if let Some(id) = input.get("id").and_then(|v| v.as_str()) {
        return Ok(RunSource::ById(id.to_string()));
    }
    let lang = str_arg(input, "lang")?;
    let text = str_arg(input, "text")?;
    let target = str_arg(input, "target")?;
    let params = str_array(input, "params");
    Ok(RunSource::Inline {
        lang: lang.to_string(),
        text: text.to_string(),
        target: target.to_string(),
        params,
    })
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}

fn str_array(input: &Value, key: &str) -> Vec<String> {
    input
        .get(key)
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

/// Parse the optional `vars` object into `$`-bound query bindings (mirrors `store.query`'s `vars`).
fn vars_arg(input: &Value) -> Vec<(String, Value)> {
    match input.get("vars") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(o)) => o.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => Vec::new(),
    }
}
