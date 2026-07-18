//! The `pack.*` MCP bridge — shape in, shape out, and nothing else. Authority lives in the verbs.
//!
//! The bundle arrives as `{manifest, files}` in the call itself (pack-core-scope
//! §"Bundle-over-the-wire"), so a third party can apply a pack over MCP with nothing but a session
//! and caps. `typed_arg` tolerates the JSON-encoded-string form AI callers routinely emit, the same
//! leniency `dashboard.save` needed for `cells` — it costs no authority, since the verb's own
//! resolve/lint/gate chain still runs on the decoded value.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_packs::Bundle;
use serde_json::Value;

use super::error::PackError;
use super::read::{pack_get, pack_list};
use super::validate::pack_validate;
use super::verb::pack_apply;
use crate::boot::Node;

/// Dispatch one `pack.<verb>` call.
pub async fn call_pack_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "pack.validate" => {
            let bundle = bundle_arg(input)?;
            Ok(pack_validate(&node.store, principal, ws, &bundle).await?)
        }
        "pack.apply" => {
            let bundle = bundle_arg(input)?;
            // The logical apply time is the caller's clock (determinism — never a wall clock in the
            // verb), defaulting to 0 so a deterministic caller that omits it is reproducible.
            let ts = input
                .get("ts")
                .and_then(|v| {
                    v.as_u64()
                        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
                })
                .unwrap_or(0);
            Ok(pack_apply(node, principal, ws, &bundle, ts).await?)
        }
        "pack.list" => Ok(pack_list(&node.store, principal, ws).await?),
        "pack.get" => {
            let pack = input
                .get("pack")
                .and_then(Value::as_str)
                .ok_or_else(|| ToolError::BadInput("missing arg: pack".into()))?;
            Ok(pack_get(&node.store, principal, ws, pack).await?)
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Decode the `bundle` arg, tolerating the JSON-encoded-string form.
fn bundle_arg(input: &Value) -> Result<Bundle, PackError> {
    let raw = input
        .get("bundle")
        .ok_or_else(|| PackError::BadInput("missing arg: bundle".into()))?;
    let value = match raw {
        Value::String(s) => serde_json::from_str::<Value>(s).map_err(|_| {
            PackError::BadInput(
                "bundle: arrived as a string that is not valid JSON — pass a JSON object \
                 {manifest, files}, not a JSON-encoded string"
                    .into(),
            )
        })?,
        other => other.clone(),
    };
    serde_json::from_value(value)
        .map_err(|e| PackError::BadInput(format!("bundle: {e} (expected {{manifest, files}})")))
}
