//! `POST /mcp/call {tool, args}` — the universal host-mediated bridge. Every
//! platform verb that isn't wrapped by name in this library is reachable from
//! here without a library update (see `skills/ingest-series/SKILL.md` for the
//! verb table). Re-checks the workspace + `mcp:<tool>:call` capability.

use reqwest::Method;
use serde::Serialize;

use crate::client::{decode, Client};
use crate::error::LbError;
use crate::Json;

/// Call `tool` with `args` over the bridge. `args` may be `serde_json::Value` (
/// e.g. `json!({"series": "node.cpu_temp"})`) or any `Serialize` struct; pass
/// `Json::Null` for a no-arg tool. Returns the tool's raw JSON output.
pub async fn call_mcp<A: Serialize>(
    client: &Client,
    tool: &str,
    args: &A,
) -> Result<Json, LbError> {
    let args = serde_json::to_value(args).unwrap_or(Json::Null);
    let body = serde_json::json!({ "tool": tool, "args": args });
    let resp = client
        .request(Method::POST, "/mcp/call")
        .json(&body)
        .send()
        .await?;
    decode(resp).await
}
