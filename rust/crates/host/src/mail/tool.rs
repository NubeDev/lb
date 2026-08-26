//! The `mail.source.*` MCP bridge — host-native verbs under the one MCP contract (rule 7).
//!
//! The UI roster, an agent, a flow node, and `curl` all reach a mail source the same way. Each verb
//! re-authorizes inside (`authorize_mail_source`), so the outer gate in `tool_call.rs` and the inner
//! one ask the same question; a denial is opaque.
//!
//! **The source is the arg object itself**, flat — `{id, host, username, secretPath, …}` — not nested
//! under a `source` key. This once accepted both, which was wrong in a way only a live call revealed:
//! the descriptor's schema (`descriptor::register_schema`) declares the flat fields and
//! `tools::validate_args` enforces it BEFORE dispatch, so a nested body was rejected with
//! "missing required arg: id" by the validator and never reached the tolerant parse. One shape, and
//! the descriptor is the contract.
//!
//! There is deliberately **no `mail.source.import` verb** that takes raw bytes. The import path is
//! reachable only by a message actually arriving in a registered mailbox, under the narrow importer
//! principal — a verb that let a caller hand the platform arbitrary bytes to "import as mail" would
//! be a way to write assets, inbox items, and series data while holding none of those caps.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::check::mail_source_check;
use super::error::MailSourceError;
use super::list::{mail_source_get, mail_source_list};
use super::poll::poll_source;
use super::register::mail_source_register;
use super::remove::{mail_source_delete, mail_source_pause};
use super::source::MailSource;
use super::store::read_source;

/// Dispatch one `mail.source.<verb>` call.
pub async fn call_mail_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
    now: u64,
) -> Result<Value, ToolError> {
    let tokens = super::fetcher::token_cache();
    let http = super::fetcher::http_client();
    match qualified_tool {
        "mail.source.register" | "mail.source.update" => {
            let source: MailSource = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("source: {e}")))?;
            let saved = mail_source_register(store, principal, ws, source, now)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "source": saved }))
        }
        "mail.source.list" => {
            let sources = mail_source_list(store, principal, ws)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "sources": sources }))
        }
        "mail.source.get" => {
            let id = str_arg(input, "id")?;
            let source = mail_source_get(store, principal, ws, id)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "source": source }))
        }
        "mail.source.delete" => {
            let id = str_arg(input, "id")?;
            mail_source_delete(store, principal, ws, id)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "mail.source.pause" => {
            let id = str_arg(input, "id")?;
            // `paused` defaults to TRUE: the verb is named `pause`, and an operator reaching for it
            // in an incident should not have to pass an argument to make it stop.
            let paused = input.get("paused").and_then(Value::as_bool).unwrap_or(true);
            let source = mail_source_pause(store, principal, ws, id, paused)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "source": source }))
        }
        "mail.source.check" => {
            let id = str_arg(input, "id")?;
            let result = mail_source_check(store, principal, ws, id, tokens, http)
                .await
                .map_err(to_tool)?;
            Ok(json!({ "check": result }))
        }
        "mail.source.poll" => {
            // The on-demand pass. Gated on its own cap because it spends an external connection and
            // ADVANCES the cursor — unlike `check`, this one really imports.
            let id = str_arg(input, "id")?;
            super::authorize::authorize_mail_source(principal, ws, "poll").map_err(to_tool)?;
            let mut source = read_source(store, ws, id)
                .await
                .map_err(|e| ToolError::Extension(e.to_string()))?
                .ok_or_else(|| to_tool(MailSourceError::NotFound))?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .unwrap_or(super::reactor::MAIL_BATCH as u64) as usize;
            let fetcher = super::fetcher::build_fetcher(store, ws, &source, tokens, http)
                .await
                .map_err(to_tool)?;
            let pass = poll_source(store, ws, &mut source, fetcher.as_ref(), limit, now)
                .await
                .map_err(to_tool)?;
            Ok(json!({
                "pass": {
                    "source": pass.source,
                    "fetched": pass.fetched,
                    "imported": pass.imported,
                    "duplicates": pass.duplicates,
                    "rejected": pass.rejected,
                    "failed": pass.failed,
                    "samples": pass.samples,
                    "series": pass.series,
                    "more": pass.more,
                    "error": pass.error,
                }
            }))
        }
        "mail.formats" => {
            // The decoder registry, read out rather than hardcoded in a UI. Grant-free: it is a
            // static list of capabilities of this binary, with no workspace data in it.
            Ok(json!({
                "formats": lb_ingest::FORMATS
                    .iter()
                    .map(|f| json!({ "id": f.id, "description": f.description }))
                    .collect::<Vec<_>>()
            }))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn str_arg<'a>(input: &'a Value, name: &str) -> Result<&'a str, ToolError> {
    input
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput(format!("missing/!string arg: {name}")))
}

/// Map the service error onto the MCP surface. A denial stays opaque; everything else keeps its
/// reason, because an operator configuring a mailbox needs to see it.
fn to_tool(error: MailSourceError) -> ToolError {
    match error {
        MailSourceError::Denied => ToolError::Denied,
        MailSourceError::NotFound => ToolError::NotFound,
        MailSourceError::BadInput(m) => ToolError::BadInput(m),
        MailSourceError::Transport { message, .. } => ToolError::Extension(message),
        MailSourceError::Store(e) => ToolError::Extension(e.to_string()),
    }
}
