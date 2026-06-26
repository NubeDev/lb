//! The MCP bridge for asset verbs — host-native tools under the one MCP contract (README §6.5,
//! "MCP is the universal contract", §3.7). UI, AI agents, and extensions all reach the asset
//! verbs the SAME way they reach a wasm tool: a qualified `assets.<verb>` call with JSON in/out.
//!
//! Two gates, in order, exactly like every other tool call:
//!   1. the **MCP gate** — `mcp::authorize_tool` (workspace-first, then `mcp:assets.<verb>:call`).
//!      This is what makes the mandatory MCP-surface isolation + deny tests real: a ws-B caller
//!      (or one without the `mcp:assets.*:call` grant) is refused HERE, before the verb runs.
//!   2. the **asset gate** — the verb itself re-checks the `store:doc/*`/`store:skill/*`
//!      capability and the membership/grant gate (`assets/*`). Two independent surfaces, both
//!      enforced — an MCP grant never bypasses the store/membership check.
//!
//! Host-native (not a wasm extension), so it is NOT in the runtime `Registry`; the gateway/UI
//! route `assets.*` here. Tool input/output are small JSON objects (the contract snapshot lives
//! in the mcp/files scope docs).

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use lb_store::Store;
use serde_json::{json, Value};

use super::{
    get_doc, grant_skill, link_doc, list_docs, load_skill, put_doc, put_skill, share_doc,
    AssetError,
};

/// Dispatch an `assets.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Authorization (the MCP gate) runs first; the verb adds its own gate.
pub async fn call_asset_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // Gate 1: the MCP surface — workspace-first, then mcp:assets.<verb>:call. Denials here are
    // opaque (no existence signal), the same contract as a routed extension call.
    authorize_tool(principal, ws, qualified_tool)?;

    let verb = qualified_tool
        .split_once('.')
        .map(|(_, v)| v)
        .unwrap_or(qualified_tool);

    let out = match verb {
        "put_doc" => {
            let d = put_doc(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                str_arg(input, "content")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(asset_to_tool)?;
            json!({ "id": d.id })
        }
        "get_doc" => {
            let d = get_doc(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(asset_to_tool)?;
            json!({ "id": d.id, "title": d.title, "content": d.content, "owner": d.owner })
        }
        "list_docs" => {
            let docs = list_docs(store, principal, ws)
                .await
                .map_err(asset_to_tool)?;
            json!({ "docs": docs.iter().map(|d| json!({"id": d.id, "title": d.title})).collect::<Vec<_>>() })
        }
        "share_doc" => {
            share_doc(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "team")?,
            )
            .await
            .map_err(asset_to_tool)?;
            json!({ "ok": true })
        }
        "link_doc" => {
            link_doc(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "channel")?,
            )
            .await
            .map_err(asset_to_tool)?;
            json!({ "ok": true })
        }
        "put_skill" => {
            let s = put_skill(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "version")?,
                str_arg(input, "description")?,
                str_arg(input, "body")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(asset_to_tool)?;
            json!({ "id": s.id, "version": s.version })
        }
        "grant_skill" => {
            grant_skill(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(asset_to_tool)?;
            json!({ "ok": true })
        }
        "load_skill" => {
            let version = input.get("version").and_then(|v| v.as_str());
            let s = load_skill(store, principal, ws, str_arg(input, "id")?, version)
                .await
                .map_err(asset_to_tool)?;
            json!({ "id": s.id, "version": s.version, "body": s.body })
        }
        _ => return Err(ToolError::NotFound),
    };
    Ok(out)
}

/// Map the asset gate's outcome onto the MCP tool error. A `Denied`/`NotFound` from the asset
/// layer stays a `Denied`/`NotFound` — the MCP caller cannot distinguish the two gates, so the
/// store/membership deny leaks no more than the MCP deny did.
fn asset_to_tool(e: AssetError) -> ToolError {
    match e {
        AssetError::Denied => ToolError::Denied,
        AssetError::NotFound => ToolError::NotFound,
        AssetError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput(format!("missing u64 arg: {key}")))
}
