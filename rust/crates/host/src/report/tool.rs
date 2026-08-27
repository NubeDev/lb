//! The MCP bridge for report verbs — host-native tools under the one MCP contract (reports scope).
//! UI, agents, and extensions reach `report.*` the same way as any wasm tool: a qualified call with
//! JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first, then `mcp:report.<verb>:
//! call`). `save`/`delete`/`share` take their logical `now` from the args (the caller's clock).
//!
//! `report.export` **is** here now, and it is the one verb in this family that does not take and
//! return what its gateway sibling does. The route is a binary one — snapshot PNGs up in the body,
//! `%PDF` bytes down — and neither half fits the JSON MCP envelope, so the bridge arm trades
//! **media ids** instead: `{ id, snapshotMediaId? }` → `{ pdfMediaId, bytes, mime }`, with the
//! snapshots riding up through the shipped `media.upload_*` path and the PDF walking down through
//! the shipped `media.read`. The route stays the primary path for any caller that can set an
//! `Authorization` header; this is the narrow exception for the ones that cannot — a
//! module-federated extension UI — exactly as `media/read.rs` is. See [`super::export_media`] for
//! the whole argument and the measurement (the `/mcp/call` 2 MiB cap) that decided the shape.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::model::{Block, Visibility};
use super::options::ExportOptions;
use super::{
    report_delete, report_export_media, report_get, report_list, report_save, report_share,
    ReportError,
};

/// Dispatch a `report.<verb>` MCP call. Each verb authorizes first; denials are opaque.
pub async fn call_report_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "report.get" => {
            let r = report_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(to_tool)?;
            Ok(serde_json::to_value(r).unwrap_or(Value::Null))
        }
        "report.list" => {
            let rows = report_list(store, principal, ws).await.map_err(to_tool)?;
            Ok(json!({ "reports": rows }))
        }
        "report.save" => {
            let blocks: Vec<Block> = serde_json::from_value(arg(input, "blocks")?.clone())
                .map_err(|e| ToolError::BadInput(format!("blocks: {e}")))?;
            let r = report_save(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "title")?,
                blocks,
                input.get("brandId").and_then(Value::as_str).unwrap_or(""),
                input.get("toolbar").cloned().unwrap_or(Value::Null),
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(r).unwrap_or(Value::Null))
        }
        "report.delete" => {
            report_delete(
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
        "report.share" => {
            let visibility = visibility_arg(input)?;
            let team = input.get("team").and_then(|v| v.as_str());
            let r = report_share(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                visibility,
                team,
                u64_arg(input, "now")?,
            )
            .await
            .map_err(to_tool)?;
            Ok(serde_json::to_value(r).unwrap_or(Value::Null))
        }
        "report.export" => {
            // `snapshotMediaId` is OPTIONAL: composing with no captures places a titled error tile
            // in every cell and leaves the page count unchanged, which is a useful shape (the
            // report's skeleton) and never a silent success.
            let snapshot_media_id = input.get("snapshotMediaId").and_then(Value::as_str);
            // The caller's clock, as every other verb in this family takes it — but defaulted
            // rather than required, matching `media.*`'s own arm: the media record this stores is
            // the only thing that reads it, and a caller that omits it gets an epoch-zero
            // `created_ts` rather than a refusal it cannot act on.
            let now = input.get("now").and_then(Value::as_u64).unwrap_or(0);
            // The page contract, additive: an older client sends no `options` key at all and gets
            // `ExportOptions::default()`, which is the document that shipped. A malformed one is a
            // named BadInput rather than a silent fall back to A4 — a caller who asked for Letter and
            // quietly got A4 has a wrong PDF they cannot see.
            let options: ExportOptions = match input.get("options") {
                None | Some(Value::Null) => ExportOptions::default(),
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| {
                    to_tool(ReportError::BadInput(format!("options is malformed: {e}")))
                })?,
            };
            report_export_media(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                snapshot_media_id,
                &options,
                now,
            )
            .await
            .map_err(to_tool)
        }
        _ => Err(ToolError::NotFound),
    }
}

fn to_tool(e: ReportError) -> ToolError {
    match e {
        ReportError::Denied => ToolError::Denied,
        ReportError::NotFound => ToolError::NotFound,
        ReportError::BadInput(m) => ToolError::BadInput(m),
        ReportError::Render(m) => ToolError::Extension(m),
        ReportError::Store(s) => ToolError::Extension(s.to_string()),
        ReportError::Media(m) => ToolError::Extension(m),
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

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    arg(input, key)?
        .as_u64()
        .ok_or_else(|| ToolError::BadInput(format!("arg not a u64: {key}")))
}

fn visibility_arg(input: &Value) -> Result<Visibility, ToolError> {
    match str_arg(input, "visibility")? {
        "private" => Ok(Visibility::Private),
        "team" => Ok(Visibility::Team),
        "workspace" => Ok(Visibility::Workspace),
        other => Err(ToolError::BadInput(format!("bad visibility: {other}"))),
    }
}
