//! The MCP bridge for ingest verbs — host-native tools under the one MCP contract (README §6.5).
//! UI, agents, and producers reach `ingest.write` / `series.read` / `series.latest` the SAME way
//! they reach any wasm tool: a qualified call with JSON in/out. The MCP gate (`authorize_ingest`)
//! runs inside each verb FIRST — a ws-B caller, or one without the grant, is refused before the
//! verb runs (the mandatory MCP-surface deny + isolation tests are real here).
//!
//! Host-native (not a wasm extension), so it is NOT in the runtime `Registry`; the gateway/UI route
//! `ingest.*` / `series.*` here.

use lb_auth::Principal;
use lb_ingest::Sample;
use lb_mcp::ToolError;
use lb_store::Store;
use lb_tags::Facet;
use serde_json::{json, Value};

use super::{drain_workspace, ingest_write, series_latest_value, series_read_range, IngestError};

/// Dispatch an ingest/series MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb authorizes first; denials are opaque (`ToolError::Denied`).
pub async fn call_ingest_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "ingest.write" => {
            let samples: Vec<Sample> = serde_json::from_value(arg(input, "samples")?.clone())
                .map_err(|e| ToolError::BadInput(format!("samples: {e}")))?;
            let n = ingest_write(store, principal, ws, samples)
                .await
                .map_err(ingest_to_tool)?;
            // Drain staging → the committed `series` table so the just-written sample is visible to the
            // very next `series.latest`/`read` over THIS same bridge — the round-trip the proof-panel
            // page proves. There is no background drain worker; the gateway's own `POST /ingest` route
            // drains synchronously for the same reason. The drain is exactly-once per
            // `(series, producer, seq)`, so a write-then-read never double-commits.
            drain_workspace(store, ws).await.map_err(ingest_to_tool)?;
            Ok(json!({ "accepted": n }))
        }
        "series.read" => {
            let series = str_arg(input, "series")?;
            // Open bounds when omitted — never a `u64::MAX` sentinel (it coerces to a float and the
            // comparison mis-evaluates; see debugging/ingest/u64-max-bound-coerces-to-float.md).
            let from = u64_arg(input, "from_seq");
            let to = u64_arg(input, "to_seq");
            let rows = series_read_range(store, principal, ws, series, from, to)
                .await
                .map_err(ingest_to_tool)?;
            Ok(json!({ "samples": rows }))
        }
        "series.latest" => {
            let series = str_arg(input, "series")?;
            let last = series_latest_value(store, principal, ws, series)
                .await
                .map_err(ingest_to_tool)?;
            Ok(json!({ "sample": last }))
        }
        "series.find" => {
            let facets = facets(input)?;
            let hits = super::series_find(store, principal, ws, &facets)
                .await
                .map_err(ingest_to_tool)?;
            Ok(json!({ "series": hits }))
        }
        "series.list" => {
            // Prefix is optional — absent/empty lists every series.
            let prefix = input.get("prefix").and_then(|v| v.as_str()).unwrap_or("");
            let names = super::series_list(store, principal, ws, prefix)
                .await
                .map_err(ingest_to_tool)?;
            Ok(json!({ "series": names }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the ingest gate's outcome onto the MCP tool error. `Denied` stays `Denied` (no existence
/// signal); a store/input error surfaces as `Extension`/`BadInput`.
fn ingest_to_tool(e: IngestError) -> ToolError {
    match e {
        IngestError::Denied => ToolError::Denied,
        IngestError::BadInput(m) => ToolError::BadInput(m),
        IngestError::Store(s) => ToolError::Extension(s.to_string()),
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

fn u64_arg(input: &Value, key: &str) -> Option<u64> {
    input.get(key).and_then(|v| v.as_u64())
}

/// Parse the `facets` array of a `series.find` query (value present → exact, absent → key-only).
fn facets(input: &Value) -> Result<Vec<Facet>, ToolError> {
    let arr = input
        .get("facets")
        .and_then(|v| v.as_array())
        .ok_or_else(|| ToolError::BadInput("missing facets array".into()))?;
    arr.iter()
        .map(|f| {
            let key = f
                .get("key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::BadInput("facet missing key".into()))?;
            Ok(match f.get("value") {
                Some(v) => Facet::exact(key, v.clone()),
                None => Facet::key_only(key),
            })
        })
        .collect()
}
