//! The bus MCP bridge — the one MCP contract over `bus.publish` (widget-config-vars scope, "Platform
//! fix"). `bus.publish` is a fire-and-forget call reachable via `POST /mcp/call`; `bus.watch` is a
//! STREAM reached via the gateway SSE route (`GET /bus/{subject}/stream`), not this synchronous dispatch
//! — calling `bus.watch` here is a `BadInput` (use the stream). Mirrors `call_ingest_tool`.

use lb_auth::Principal;
use lb_bus::Bus;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::error::BusError;
use super::publish::bus_publish;

fn bus_to_tool(e: BusError) -> ToolError {
    match e {
        BusError::Denied => ToolError::Denied,
        BusError::BadSubject(m) | BusError::BadInput(m) => ToolError::BadInput(m),
        BusError::Bus(m) => ToolError::Extension(m),
    }
}

/// Dispatch a `bus.*` MCP call. `bus.publish` publishes a JSON payload onto a walled subject (fire-and-
/// forget → `{ ok: true }`). `bus.watch` is stream-only (use the SSE route).
pub async fn call_bus_tool(
    bus: &Bus,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "bus.publish" => {
            let subject = input
                .get("subject")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("subject (string) required".into()))?;
            // The payload is an opaque JSON value (default `{}`); serialized as-is onto the subject.
            let payload = input.get("payload").cloned().unwrap_or_else(|| json!({}));
            let bytes = serde_json::to_vec(&payload)
                .map_err(|e| ToolError::BadInput(format!("payload: {e}")))?;
            bus_publish(bus, principal, ws, subject, &bytes)
                .await
                .map_err(bus_to_tool)?;
            // Fire-and-forget: `ok` means "handed to the bus", NEVER "delivered" (rule 3).
            Ok(json!({ "ok": true }))
        }
        "bus.watch" => Err(ToolError::BadInput(
            "bus.watch is a stream — use GET /bus/{subject}/stream".into(),
        )),
        _ => Err(ToolError::NotFound),
    }
}
