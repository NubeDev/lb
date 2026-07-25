//! The MCP bridge for form verbs — host-native tools under the one MCP contract. UI, agents, and
//! extensions reach `forms.*` the SAME way they reach any wasm tool: a qualified call with JSON
//! in/out. The MCP gate runs inside each verb FIRST (workspace-first, then `mcp:forms.<verb>:call`),
//! so a ws-B caller or one without the grant is refused before the verb runs. Host-native — the
//! gateway routes `forms.*` here. Mirrors `dashboard`'s bridge.
//!
//! `save`/`delete` take their logical `now` from the args (the caller's clock — determinism, never
//! wall-clock in the verb).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::{forms_delete, forms_get, forms_list, forms_save, FormError};

/// Dispatch a `forms.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the verb's
/// JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_forms_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "forms.get" => {
            let f = forms_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(f).unwrap_or(Value::Null))
        }
        "forms.list" => {
            let rows = forms_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "forms": rows }))
        }
        "forms.save" => {
            let def: Value = typed_arg(arg(input, "def")?, "def")?;
            let f = forms_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                def,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(f).unwrap_or(Value::Null))
        }
        "forms.delete" => {
            forms_delete(
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

/// Map the form gate's outcome onto the MCP tool error (denials opaque).
fn to_tool(e: FormError) -> ToolError {
    match e {
        FormError::Denied => ToolError::Denied,
        FormError::NotFound => ToolError::NotFound,
        FormError::BadInput(m) => ToolError::BadInput(m),
        FormError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn arg<'a>(input: &'a Value, key: &str) -> Result<&'a Value, ToolError> {
    input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

/// Decode a structured arg, tolerating the JSON-encoded-STRING form (`"{…}"` instead of `{…}`) AI
/// callers routinely emit (the `dashboard.save` cells regression). Decoding the string costs nothing
/// in authority — the definition is opaque and persisted as-is. A string that is not valid JSON of
/// the target type errors with a message that names the right encoding.
fn typed_arg<T: serde::de::DeserializeOwned>(v: &Value, key: &str) -> Result<T, ToolError> {
    let v = match v {
        Value::String(s) => serde_json::from_str::<Value>(s).map_err(|_| {
            ToolError::BadInput(format!(
                "{key}: arrived as a string that is not valid JSON — pass a JSON object, not a JSON-encoded string"
            ))
        })?,
        other => other.clone(),
    };
    serde_json::from_value(v).map_err(|e| ToolError::BadInput(format!("{key}: {e}")))
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    arg(input, key)?
        .as_str()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a string: {key}")))
}

/// A u64 arg, tolerating the numeric-STRING form (`"1783235133"`) AI callers routinely emit. The
/// steering message names the expected encoding.
fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    let v = arg(input, key)?;
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
        .ok_or_else(|| {
            ToolError::BadInput(format!(
                "arg not a u64: {key} — pass unix epoch seconds as a JSON number"
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// `def` as a real object decodes as before.
    #[test]
    fn typed_arg_decodes_a_real_object() {
        let def: Value = typed_arg(&json!({ "schema": {} }), "def").expect("object decodes");
        assert_eq!(def["schema"], json!({}));
    }

    /// `def` as a JSON-ENCODED STRING (the AI caller shape) decodes to the same value.
    #[test]
    fn typed_arg_tolerates_a_json_encoded_string() {
        let def: Value = typed_arg(&json!("{\"schema\":{}}"), "def").expect("stringified decodes");
        assert_eq!(def["schema"], json!({}));
    }

    /// A string that is not valid JSON errors with a message that names the right encoding.
    #[test]
    fn typed_arg_steers_on_a_non_json_string() {
        let err = typed_arg::<Value>(&json!("not json"), "def").unwrap_err();
        let ToolError::BadInput(msg) = err else {
            panic!("expected BadInput")
        };
        assert!(
            msg.contains("JSON-encoded string"),
            "steering message: {msg}"
        );
    }
}
