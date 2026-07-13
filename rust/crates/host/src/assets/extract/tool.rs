//! The `docs.*` MCP bridge (doc-extraction scope) — the host-native dispatcher for the `docs.`
//! verb family, reached the SAME way every host-native verb is (rule 7): a qualified `docs.<verb>`
//! call with JSON in/out. v1 has one verb, `docs.extract`; the embeddings scope adds `docs.search`
//! / `docs.reindex` under this same prefix later (which is why `docs.` is its own namespace, not an
//! arm of the `assets.` asset-CRUD bridge).
//!
//! Two gates, in order, exactly like the sibling bridges:
//!   1. the MCP gate — `authorize_tool` (workspace-first, then `mcp:docs.extract:call`), also run
//!      by the outer dispatcher; re-run here so this bridge is safe called directly (defense in
//!      depth, and the tested seam).
//!   2. the service gate — the verb re-checks its own cap AND per-item media read reach + doc write.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolDescriptor, ToolError};
use lb_store::Store;
use serde_json::{json, Value};

use super::model::{ExtractRequest, ItemOutcome};
use super::{docs_extract, ExtractResult};

/// Dispatch a `docs.<verb>` MCP call. Today: `docs.extract`.
pub async fn call_docs_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, qualified_tool)?;

    let verb = qualified_tool
        .split_once('.')
        .map(|(_, v)| v)
        .unwrap_or(qualified_tool);

    match verb {
        "extract" => {
            let req = parse_request(input)?;
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            let result = docs_extract(store, principal, ws, &req, ts)
                .await
                .map_err(ToolError::from)?;
            Ok(render_result(&result))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// The `docs.extract` descriptor — the palette/agent-facing schema + defense-in-depth arg
/// validation (channels-command-palette scope). Gated on `mcp:docs.extract:call` via the name.
/// Generic over mime (rule 10): the form takes media ids + caller doc fields, never a domain noun.
pub fn extract_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        emits_external: false,
        name: "docs.extract".to_string(),
        title: "Extract markdown docs from media".to_string(),
        group: "docs".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                // A single id (string) OR an array of ids — the "one id, watch the job"
                // convenience the scope names. Deliberately un-`type`d so the descriptor validator
                // accepts both forms (it does a single-type shallow check); the handler is
                // authoritative and normalizes both.
                "media": {
                    "items": { "type": "string" },
                    "description": "source media id, or an array of ids, to extract"
                },
                "title": { "type": "string", "description": "title override for the derived doc(s)" },
                "tags": { "type": "array", "items": { "type": "string" } },
                "split": {
                    "type": "string",
                    "enum": ["whole", "per_part"],
                    "description": "multi-part sources (a workbook): one doc or one per sheet"
                },
                "force_version": {
                    "type": "integer",
                    "description": "re-derive at/under this extractor version (model migration)"
                }
            },
            "required": ["media"]
        })),
        result: None,
    }
}

/// Parse the `docs.extract` JSON arguments. `media` accepts a single string id or an array of ids
/// (the "one id, watch the job" convenience the scope names). `split` is `"whole"` (default) or
/// `"per_part"`. `title`/`tags`/`force_version` are optional.
fn parse_request(input: &Value) -> Result<ExtractRequest, ToolError> {
    let media = match input.get("media") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => {
            return Err(ToolError::BadInput(
                "missing 'media' (string or array of ids)".into(),
            ))
        }
    };
    if media.is_empty() {
        return Err(ToolError::BadInput("'media' resolved to no ids".into()));
    }
    let split = match input.get("split").and_then(Value::as_str) {
        Some("per_part") => lb_extract::SplitPolicy::PerPart,
        Some("whole") | None => lb_extract::SplitPolicy::Whole,
        Some(other) => {
            return Err(ToolError::BadInput(format!(
                "unknown split policy {other:?} (expected 'whole' or 'per_part')"
            )))
        }
    };
    let title = input
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string);
    let tags = input
        .get("tags")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let force_version = input
        .get("force_version")
        .and_then(Value::as_u64)
        .map(|v| v as u32);
    Ok(ExtractRequest {
        media,
        title,
        tags,
        split,
        force_version,
    })
}

/// Render the batch result to JSON: the job id + one object per item (`status` tag + fields), so a
/// caller sees exactly which media extracted, which were unsupported/failed, and which were denied.
fn render_result(result: &ExtractResult) -> Value {
    let items: Vec<Value> = result.items.iter().map(item_json).collect();
    json!({ "job_id": result.job_id, "items": items })
}

fn item_json(o: &ItemOutcome) -> Value {
    match o {
        ItemOutcome::Extracted {
            media_id,
            doc_ids,
            reused,
        } => {
            json!({ "status": "extracted", "media_id": media_id, "doc_ids": doc_ids, "reused": reused })
        }
        ItemOutcome::Unsupported { media_id, reason } => {
            json!({ "status": "unsupported", "media_id": media_id, "reason": reason })
        }
        ItemOutcome::Failed { media_id, reason } => {
            json!({ "status": "failed", "media_id": media_id, "reason": reason })
        }
        ItemOutcome::Denied { media_id } => json!({ "status": "denied", "media_id": media_id }),
    }
}
