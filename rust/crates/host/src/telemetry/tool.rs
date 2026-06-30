//! The `telemetry.*` MCP bridge (telemetry-console scope, README §6.5/§7: the read surface is
//! reached as MCP tools under the one contract). The UI, the agent, and other extensions reach the
//! capped ring the SAME way they reach any tool — a qualified call with JSON in/out. Each verb
//! authorizes first (opaque `Denied`); the `ws` wall is in the read itself.
//!
//! Verbs: `telemetry.query` (snapshot, paged), `telemetry.trace` (one trace), `telemetry.tail`
//! (declared here; the live feed rides the SSE route, which calls [`telemetry_tail`] directly), and
//! `telemetry.purge` (admin). There is **no** `telemetry.write` — writes come from the Layer only.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_telemetry::Level;
use serde_json::{json, Value};

use super::filter::QueryFilter;
use crate::boot::Node;
use crate::telemetry::{telemetry_purge, telemetry_query, telemetry_trace};

/// Dispatch a `telemetry.*` MCP call. `input` is the verb's JSON args; the return is the verb's JSON
/// result. Each verb authorizes first (opaque `Denied`); the `ws` wall is enforced inside the read.
pub async fn call_telemetry_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "telemetry.query" => {
            let filter = parse_filter(input)?;
            let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let cursor = input.get("cursor").and_then(|v| v.as_str());
            let page = telemetry_query(&node.store, principal, ws, &filter, limit, cursor)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "rows": page.rows, "next": page.next }))
        }
        "telemetry.trace" => {
            let trace_id = str_arg(input, "trace_id")?;
            let rows = telemetry_trace(&node.store, principal, ws, trace_id)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "rows": rows }))
        }
        "telemetry.purge" => {
            let n = telemetry_purge(&node.store, principal, ws)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "removed": n }))
        }
        // `telemetry.tail` has no in-band result — the live feed rides the SSE route, which calls
        // `telemetry_tail` directly (like `agent.watch`). Declared here so the catalog lists it and
        // the dispatcher maps it to `NotFound` rather than falling through to `ingest`.
        "telemetry.tail" => Err(ToolError::NotFound),
        _ => Err(ToolError::NotFound),
    }
}

/// Parse the console's filter args into a [`QueryFilter`]. Every field optional; unknown level/
/// outcome values are a clean `BadInput` (the console's filter set is bounded).
fn parse_filter(input: &Value) -> Result<QueryFilter, ToolError> {
    let mut f = QueryFilter::default();
    if let Some(s) = input.get("source").and_then(|v| v.as_str()) {
        f.source = Some(s.to_string());
    }
    if let Some(s) = input.get("actor").and_then(|v| v.as_str()) {
        f.actor = Some(s.to_string());
    }
    if let Some(s) = input.get("level").and_then(|v| v.as_str()) {
        f.min_level = Some(
            Level::parse(s).ok_or_else(|| ToolError::BadInput(format!("unknown level: {s}")))?,
        );
    }
    if let Some(s) = input.get("outcome").and_then(|v| v.as_str()) {
        f.outcome = Some(s.to_string());
    }
    if let Some(s) = input.get("trace_id").and_then(|v| v.as_str()) {
        f.trace_id = Some(s.to_string());
    }
    if let Some(s) = input.get("text").and_then(|v| v.as_str()) {
        f.text = Some(s.to_string());
    }
    if let Some(n) = input.get("since").and_then(|v| v.as_u64()) {
        f.since = Some(n);
    }
    if let Some(n) = input.get("until").and_then(|v| v.as_u64()) {
        f.until = Some(n);
    }
    Ok(f)
}

fn svc_to_tool(e: crate::telemetry::TelemetrySvcError) -> ToolError {
    match e {
        crate::telemetry::TelemetrySvcError::Denied => ToolError::Denied,
        crate::telemetry::TelemetrySvcError::BadInput(m) => ToolError::BadInput(m),
        other => ToolError::Extension(other.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing/invalid arg: {key}")))
}
