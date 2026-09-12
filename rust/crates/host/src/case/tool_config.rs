//! The case plane's CONFIGURATION verbs — the party roster and the SLA service policies
//! (case-plane scope §Wave 5).
//!
//! Split out of `tool.rs` when the delete verbs pushed that file past the 400-line FILE-LAYOUT
//! limit, and the seam it found is a real one rather than an arbitrary cut: everything here is
//! **admin settings about the plane**, not work done on a case. `case.*` moves a piece of work
//! through its life; `party.*` and `policy.sla.*` decide who work may be sent to and what it is
//! promised by. Different authority, different audience, different rate of change.
//!
//! All six are dispatched by EXACT name from `HOST_NATIVE_EXACT` (never a `policy.`/`party.` prefix
//! — the host does not reserve a whole namespace against an extension that might legitimately be
//! called `party`), and each gates on its own name, so none needs a `tool_gate.rs` arm.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::boot::Node;
use crate::case::{
    case_party_delete, case_party_list, case_party_upsert, case_policy_sla_delete,
    case_policy_sla_list, case_policy_sla_set,
};

use super::tool::{flag, opt_enum_arg, str_arg, svc_to_tool};

/// Dispatch one `party.*` / `policy.sla.*` call. `None` ⇒ not one of ours, and `tool.rs` carries on.
pub async fn call_case_config_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Option<Result<Value, ToolError>> {
    let store = &node.store;
    Some(match qualified_tool {
        "policy.sla.set" => policy_set(store, principal, ws, input).await,
        "policy.sla.list" => policy_list(store, principal, ws, input).await,
        "policy.sla.delete" => policy_delete(store, principal, ws, input).await,
        "party.upsert" => party_upsert(store, principal, ws, input).await,
        "party.list" => party_list(store, principal, ws, input).await,
        "party.delete" => party_delete(store, principal, ws, input).await,
        _ => return None,
    })
}

async fn policy_set(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let policy: lb_cases::ServicePolicy = serde_json::from_value(input.clone())
        .map_err(|e| ToolError::BadInput(format!("policy.sla.set: {e}")))?;
    case_policy_sla_set(store, principal, ws, &policy)
        .await
        .map_err(svc_to_tool)?;
    Ok(json!({ "id": policy.id }))
}

async fn policy_list(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // `include_disabled` defaults to FALSE: the common caller wants the policies in force. The
    // settings surface asks for true, because an admin cannot re-enable a row they cannot see.
    let policies = case_policy_sla_list(store, principal, ws, flag(input, "include_disabled"))
        .await
        .map_err(svc_to_tool)?;
    Ok(serde_json::to_value(policies).unwrap_or(Value::Null))
}

async fn policy_delete(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let id = str_arg(input, "id")?;
    let removed = case_policy_sla_delete(store, principal, ws, id)
        .await
        .map_err(svc_to_tool)?;
    Ok(json!({ "id": id, "removed": removed }))
}

async fn party_upsert(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let party: lb_cases::Party = serde_json::from_value::<super::PartyInput>(input.clone())
        .map_err(|e| ToolError::BadInput(format!("party.upsert: {e}")))?
        .into();
    case_party_upsert(store, principal, ws, &party)
        .await
        .map_err(svc_to_tool)?;
    Ok(json!({ "id": party.id }))
}

async fn party_list(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let kind = opt_enum_arg(input, "kind")?;
    let parties = case_party_list(
        store,
        principal,
        ws,
        kind,
        input.get("site").and_then(Value::as_str),
        flag(input, "include_disabled"),
    )
    .await
    .map_err(svc_to_tool)?;
    Ok(serde_json::to_value(parties).unwrap_or(Value::Null))
}

async fn party_delete(
    store: &lb_store::Store,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let id = str_arg(input, "id")?;
    let removed = case_party_delete(store, principal, ws, id)
        .await
        .map_err(svc_to_tool)?;
    Ok(json!({ "id": id, "removed": removed }))
}
