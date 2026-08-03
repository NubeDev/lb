//! The MCP bridge for `time.range.resolve` — the read-only host verb that resolves one range
//! expression to a concrete window, so flows, rules, agents and extensions get the SAME arithmetic
//! the dashboard save/validation path uses without a private copy (relative-time-range scope,
//! Goal 3). Host-native under the one MCP contract; gated by `mcp:time.range.resolve:call` through
//! the shared `authorize_tool` chokepoint (workspace-first, then capability). Pure compute — no
//! store read, no write, no motion.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolDescriptor, ToolError};
use serde_json::{json, Value};

use super::resolve::resolve_range;

/// The `time.range.resolve` descriptor — a real arg schema so an advertised caller can FORM the
/// call (the `dashboard.save` precedent: name-only rows get guessed encodings).
pub fn resolve_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "time.range.resolve".to_string(),
        title: "Resolve a relative time-range expression to a concrete window".to_string(),
        group: "time".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "from": { "type": "string", "x-lb": { "label": "From", "description": "A range token (today, yesterday, this-month, last-3-months, …) or an endpoint (now-4h, now-1d/d, an ISO day/instant, a 13-digit epoch ms)" } },
                "to": { "type": "string", "x-lb": { "label": "To", "description": "Optional endpoint (exclusive). Omit with a range token (the token IS both ends) or to end at now" } },
                "tz": { "type": "string", "x-lb": { "label": "Timezone", "description": "Optional IANA timezone the window is computed in (e.g. Australia/Sydney); empty = UTC" } },
                "now": { "type": "integer", "x-lb": { "label": "Now (ms)", "description": "Optional clock override — unix epoch MILLISECONDS; omit for the host clock" } }
            },
            "required": ["from"]
        })),
        result: None,
    }
}

/// Dispatch a `time.<verb>` MCP call. The gate runs first (opaque `Denied`); the resolution itself
/// is pure arithmetic over the caller's `(from, to?, tz?, now?)`.
pub async fn call_timerange_tool(
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "time.range.resolve" => {
            authorize_tool(principal, ws, "time.range.resolve").map_err(|_| ToolError::Denied)?;
            let from = str_arg(input, "from")?;
            let to = input
                .get("to")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty());
            let tz = input.get("tz").and_then(Value::as_str).unwrap_or("");
            // The clock is caller-injectable (determinism §3, and what makes the verb testable);
            // absent → wall-clock, the same posture as `series.retention.set`'s `now_ms`.
            let now_ms = opt_i64(input, "now").unwrap_or_else(now_wall_ms);
            let r = resolve_range(from, to, now_ms, tz)
                .map_err(|e| ToolError::BadInput(e.to_string()))?;
            Ok(json!({
                "fromMs": r.from_ms,
                "toMs": r.to_ms,
                "fromIso": r.from_day,
                "toIso": r.to_day,
            }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key} (a string)")))
}

/// An optional i64 arg, tolerating the numeric-STRING form AI callers emit (the `u64_arg`
/// precedent in `dashboard/tool.rs`).
fn opt_i64(input: &Value, key: &str) -> Option<i64> {
    let v = input.get(key)?;
    v.as_i64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
}

/// Wall-clock epoch ms — read ONLY when the caller injects no `now` (the binary-boundary rule does
/// not apply to a runtime default the args override).
fn now_wall_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
