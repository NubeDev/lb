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
    backlinks, delete_asset, delete_doc, get_asset, get_doc, grant_skill, link_doc, list_assets,
    list_docs, load_skill, put_asset, put_doc, put_skill, share_doc, unshare_doc, AssetError,
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
            let content_type = parse_content_type(input.get("content_type"));
            let tags = parse_string_list(input.get("tags"));
            let d = put_doc(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                str_arg(input, "content")?,
                content_type,
                &tags,
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
            json!({ "id": d.id, "title": d.title, "content": d.content, "owner": d.owner,
                    "content_type": d.content_type, "tags": d.tags })
        }
        "delete_doc" => {
            delete_doc(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(asset_to_tool)?;
            json!({ "ok": true })
        }
        "list_docs" => {
            let docs = list_docs(store, principal, ws)
                .await
                .map_err(asset_to_tool)?;
            json!({ "docs": docs.iter().map(|d| json!({"id": d.id, "title": d.title})).collect::<Vec<_>>() })
        }
        "share_doc" => {
            let subject = str_arg(input, "subject").or_else(|_| str_arg(input, "team"))?;
            share_doc(store, principal, ws, str_arg(input, "id")?, subject)
                .await
                .map_err(asset_to_tool)?;
            json!({ "ok": true })
        }
        "unshare_doc" => {
            let subject = str_arg(input, "subject").or_else(|_| str_arg(input, "team"))?;
            unshare_doc(store, principal, ws, str_arg(input, "id")?, subject)
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
        "backlinks" => {
            let srcs = backlinks(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(asset_to_tool)?;
            json!({ "docs": srcs })
        }
        "put_asset" => {
            let bytes = bytes_arg(input, "bytes")?;
            let a = put_asset(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "mime")?,
                bytes,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(asset_to_tool)?;
            json!({ "id": a.id, "size": a.bytes.len() })
        }
        "get_asset" => {
            let a = get_asset(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(asset_to_tool)?;
            json!({ "id": a.id, "mime": a.mime, "owner": a.owner,
                    "bytes": serde_json::Value::String(base64_string(&a.bytes)) })
        }
        "list_assets" => {
            let assets = list_assets(store, principal, ws)
                .await
                .map_err(asset_to_tool)?;
            json!({ "assets": assets.iter().map(|a| json!({"id": a.id, "mime": a.mime, "size": a.bytes.len()})).collect::<Vec<_>>() })
        }
        "delete_asset" => {
            delete_asset(store, principal, ws, str_arg(input, "id")?)
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
        AssetError::TooLarge => ToolError::BadInput("asset too large".into()),
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

/// Accept `content_type` as a string ("markdown" | "text"); default to `text` (the S4 legacy
/// opaque content) when absent, so existing `put_doc` callers keep working unmodified.
fn parse_content_type(v: Option<&Value>) -> lb_assets::ContentType {
    use lb_assets::ContentType;
    match v.and_then(|v| v.as_str()) {
        Some("markdown") => ContentType::Markdown,
        _ => ContentType::Text,
    }
}

/// Accept `tags` as a JSON array of strings; empty when absent.
fn parse_string_list(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

/// Accept `bytes` as a base64 string (the asset payload over JSON). Required for `put_asset`.
fn bytes_arg(input: &Value, key: &str) -> Result<Vec<u8>, ToolError> {
    use base64ct::Encoding;
    let s = str_arg(input, key)?;
    base64ct::Base64::decode_vec(s)
        .map_err(|e| ToolError::BadInput(format!("invalid base64 for {key}: {e}")))
}

/// Encode the asset payload back to base64 for the JSON response.
fn base64_string(bytes: &[u8]) -> String {
    use base64ct::Encoding;
    base64ct::Base64::encode_string(bytes)
}
