//! The MCP bridge for brand verbs — host-native tools under the one MCP contract (reports scope).
//! UI, agents, and extensions reach `brand.*` the same way as any wasm tool: a qualified call with
//! JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then `mcp:brand.<verb>:
//! call`). `save`/`delete` take their logical `now` from the args (the caller's clock).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::{Colors, Fonts};
use super::{brand_delete, brand_get, brand_list, brand_save, BrandError};

/// Dispatch a `brand.<verb>` MCP call. Each verb authorizes first; denials are opaque.
pub async fn call_brand_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "brand.get" => {
            let b = brand_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(b).unwrap_or(Value::Null))
        }
        "brand.list" => {
            let rows = brand_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "brands": rows }))
        }
        "brand.save" => {
            let colors: Colors = opt_from(input, "colors")?;
            let fonts: Fonts = opt_from(input, "fonts")?;
            let b = brand_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "name")?,
                input
                    .get("logoAssetId")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                colors,
                fonts,
                input
                    .get("headerText")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                input
                    .get("footerText")
                    .and_then(Value::as_str)
                    .unwrap_or(""),
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(b).unwrap_or(Value::Null))
        }
        "brand.delete" => {
            brand_delete(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn to_tool(e: BrandError) -> ToolError {
    match e {
        BrandError::Denied => ToolError::Denied,
        BrandError::NotFound => ToolError::NotFound,
        BrandError::BadInput(m) => ToolError::BadInput(m),
        BrandError::Store(s) => ToolError::Extension(s.to_string()),
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

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    arg(input, key)?
        .as_u64()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a u64: {key}")))
}

/// Deserialize an optional nested object arg (`colors`/`fonts`), defaulting when absent.
fn opt_from<T: serde::de::DeserializeOwned + Default>(
    input: &Value,
    key: &str,
) -> Result<T, ToolError> {
    match input.get(key) {
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|e| ToolError::BadInput(format!("{key}: {e}"))),
        None => Ok(T::default()),
    }
}
