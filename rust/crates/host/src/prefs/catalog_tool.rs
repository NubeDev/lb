//! The MCP bridge for the i18n-catalog surface (i18n-catalogs scope MCP surface), mirroring the
//! shipped `prefs.*` bridge. `call_catalog_tool` dispatches `message.render` / `message.set_catalog`;
//! `prefs.catalog` is dispatched by the `prefs.*` bridge (it shares the `prefs.` prefix) and delegates
//! here. All three are GATED (a catalog carries tenant overrides) — the gate lives in the verbs.

use std::collections::BTreeMap;

use lb_auth::Principal;
use lb_bus::Bus;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::catalog_verbs::{message_render, message_set_catalog, prefs_catalog};
use super::error::PrefsSvcError;

/// Dispatch a `message.*` catalog verb (gated). `render` takes `key`/`args`/`recipient?`;
/// `set_catalog` takes `locale`/`messages` (a flat key→MF1 map).
pub async fn call_catalog_tool(
    store: &Store,
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "message.render" => {
            let key = str_arg(input, "key")?;
            let args = input.get("args").cloned().unwrap_or(Value::Null);
            let recipient = input.get("recipient").and_then(|v| v.as_str());
            let r = message_render(store, principal, ws, key, &args, recipient)
                .await
                .map_err(svc_err)?;
            Ok(json!({
                "text": r.text,
                "locale_used": r.locale_used,
                "catalog_version": r.catalog_version,
            }))
        }
        "message.set_catalog" => {
            let locale = str_arg(input, "locale")?.to_string();
            let messages = messages_arg(input)?;
            message_set_catalog(store, bus, principal, ws, &locale, messages)
                .await
                .map_err(svc_err)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Dispatch `prefs.catalog` (gated, member-level). Split out so the `prefs.*` bridge delegates here
/// without duplicating the DTO shaping.
pub async fn call_prefs_catalog_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let locale = str_arg(input, "locale")?;
    let view = prefs_catalog(store, principal, ws, locale)
        .await
        .map_err(svc_err)?;
    Ok(json!({
        "locale": view.locale,
        "catalog_version": view.catalog_version,
        "messages": view.messages,
        "has_override": view.has_override,
    }))
}

fn messages_arg(input: &Value) -> Result<BTreeMap<String, String>, ToolError> {
    let obj = input
        .get("messages")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ToolError::BadInput("missing object arg: messages".into()))?;
    let mut out = BTreeMap::new();
    for (k, v) in obj {
        let s = v
            .as_str()
            .ok_or_else(|| ToolError::BadInput(format!("messages.{k} must be a string")))?;
        out.insert(k.clone(), s.to_string());
    }
    Ok(out)
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

fn svc_err(e: PrefsSvcError) -> ToolError {
    match e {
        PrefsSvcError::Denied => ToolError::Denied,
        PrefsSvcError::BadInput(m) => ToolError::BadInput(m),
        PrefsSvcError::Store(_) => ToolError::Denied,
    }
}
