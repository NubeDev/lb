//! The MCP bridge for the per-viewer chart-preference verbs — `channel.chart_pref.get` and
//! `channel.chart_pref.set`, reached over the ONE MCP contract like any host-native verb (rule 7):
//! a qualified call with JSON in/out. The outer dispatch runs the `mcp:channel.chart_pref.<verb>:call`
//! gate (member-held); each verb then re-checks the channel `sub` gate (workspace-first), so a ws-B
//! caller or one who can't read the channel is refused opaquely before any store access.
//!
//! One responsibility: parse the verb's args, delegate to the gated verb, shape the JSON result.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::chart_pref::{chart_pref_get, chart_pref_set};
use super::error::ChannelError;

/// Dispatch a `channel.chart_pref.<verb>` MCP call. `input` is the verb's JSON arguments; the return
/// is the verb's JSON result. Denials are opaque (`ToolError::Denied`).
pub async fn call_channel_chart_pref_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "channel.chart_pref.get" => {
            let spec = chart_pref_get(
                store,
                principal,
                ws,
                str_arg(input, "channel")?,
                str_arg(input, "item")?,
            )
            .await
            .map_err(to_tool)?;
            // `spec` is `null` when the viewer never saved one — the UI falls back to the host pick.
            Ok(json!({ "spec": spec }))
        }
        "channel.chart_pref.set" => {
            let spec = input
                .get("spec")
                .filter(|v| !v.is_null())
                .ok_or_else(|| ToolError::BadInput("missing arg: spec".into()))?;
            chart_pref_set(
                store,
                principal,
                ws,
                str_arg(input, "channel")?,
                str_arg(input, "item")?,
                spec,
            )
            .await
            .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn to_tool(e: ChannelError) -> ToolError {
    match e {
        ChannelError::Denied => ToolError::Denied,
        ChannelError::NotFound => ToolError::NotFound,
        ChannelError::BadInput(msg) => ToolError::BadInput(msg),
        ChannelError::Store(s) => ToolError::Extension(s.to_string()),
        ChannelError::Bus(b) => ToolError::Extension(b.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing or non-string arg: {key}")))
}
