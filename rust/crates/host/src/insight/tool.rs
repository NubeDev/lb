//! The MCP bridge for insight verbs — host-native tools under the one MCP contract (insights
//! umbrella scope + sub-scopes). UI, agents, and extensions reach `insight.*` the SAME way they
//! reach any wasm tool: a qualified call with JSON in/out. The MCP gate runs inside each verb
//! FIRST (workspace-first, then `mcp:insight.<verb>:call`), so a ws-B caller or one without the
//! grant is refused before the verb runs (the mandatory deny + isolation tests are real here).
//!
//! Mirrors `nav/tool.rs` / `inbox`'s dispatch in `tool_call.rs`. The `now`-taking verbs
//! (`raise`/`ack`/`resolve`/`sub.create`) take their logical `now` from the args (the caller's
//! clock — determinism §3, never wall-clock in the verb).
//!
//! **STUB-state**: the per-verb dispatch arms wire end to end (the plumbing is real); the
//! underlying verbs in `lb_insights` carry `todo!()` bodies. So this bridge will compile + the
//! gate will deny correctly today; a call that reaches a stubbed body returns the `todo!()` panic
//! (surfaced as a ToolError). The implementing session replaces the bodies; the wiring is stable.

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::{json, Value};

use super::error::InsightSvcError;
use super::{
    insight_ack, insight_get, insight_list, insight_occurrences, insight_policy_get,
    insight_policy_set, insight_raise, insight_resolve, insight_sub_create, insight_sub_delete,
    insight_sub_get, insight_sub_list, insight_sub_mute,
};

/// Dispatch an `insight.<verb>` MCP call. The outer `is_host_native` gate already ran
/// `mcp:insight.<verb>:call`; each verb here re-runs it inside (defense in depth).
pub async fn call_insight_tool(
    store: &Store,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "insight.raise" => {
            let raise_input: lb_insights::RaiseInput = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("raise input: {e}")))?;
            let outcome = insight_raise(store, principal, ws, raise_input)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(outcome).unwrap_or(Value::Null))
        }
        "insight.get" => {
            let insight = insight_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(insight).unwrap_or(Value::Null))
        }
        "insight.list" => {
            let query: lb_insights::ListQuery = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("list query: {e}")))?;
            let page = insight_list(store, principal, ws, query)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(page).unwrap_or(Value::Null))
        }
        "insight.ack" => {
            insight_ack(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "insight.resolve" => {
            let note = input.get("note").and_then(|v| v.as_str());
            insight_resolve(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                note,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "insight.occurrences" => {
            let cursor = input
                .get("cursor")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            let limit = input.get("limit").and_then(|v| v.as_u64()).unwrap_or(50) as usize;
            let page = insight_occurrences(
                store,
                principal,
                ws,
                str_arg(input, "insight_id")?,
                cursor,
                limit,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(page).unwrap_or(Value::Null))
        }
        "insight.sub.create" => {
            let input_args: lb_insights::SubCreateInput = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("sub.create input: {e}")))?;
            let now = u64_arg(input, "now")?;
            let id = insight_sub_create(store, principal, ws, input_args, now)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "id": id }))
        }
        "insight.sub.list" => {
            let all = input.get("all").and_then(|v| v.as_bool()).unwrap_or(false);
            let subs = insight_sub_list(store, principal, ws, all)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "subs": subs }))
        }
        "insight.sub.get" => {
            let sub = insight_sub_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(sub).unwrap_or(Value::Null))
        }
        "insight.sub.delete" => {
            insight_sub_delete(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "insight.sub.mute" => {
            let muted = input
                .get("muted")
                .and_then(|v| v.as_bool())
                .ok_or_else(|| ToolError::BadInput("missing arg: muted".into()))?;
            insight_sub_mute(store, principal, ws, str_arg(input, "id")?, muted)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "insight.policy.get" => {
            let policy = insight_policy_get(store, principal, ws)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(policy).unwrap_or(Value::Null))
        }
        "insight.policy.set" => {
            let policy: lb_insights::Policy = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("policy.set input: {e}")))?;
            insight_policy_set(store, principal, ws, &policy)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the service error onto the MCP tool error (denials opaque).
fn svc_to_tool(e: InsightSvcError) -> ToolError {
    match e {
        InsightSvcError::Denied => ToolError::Denied,
        InsightSvcError::BadInput(m) => ToolError::BadInput(m),
        InsightSvcError::Store(s) => ToolError::Extension(s),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing or non-string arg: {key}")))
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput(format!("missing or non-u64 arg: {key}")))
}
