//! `case.request.list` — the asks raised on one case, for the drawer (case-plane scope, wave 2).
//!
//! Gated on `mcp:case.get:call` through a `tool_gate.rs` alias, beside `case.members` and
//! `case.events`: who we asked, what we asked and what came back **is** the case's detail, and a
//! reader who may open a case may see it. A separate cap would exist in no bundle and be `Denied`
//! for everyone including admins.
//!
//! It is also the one place [`super::request_delivery::reconcile_delivery`] runs: `delivery` is
//! derived from the outbox ledger on read (see that file for why it is not pushed), and this is the
//! read that renders it. A list pass therefore leaves every row's `delivery` correct as a
//! side-effect — a self-healing field, in the same spirit as the case-id echo.
//!
//! **The token hash never leaves this verb.** It is a credential, and a drawer has no use for it.

use lb_auth::Principal;
use lb_cases::CaseRequest;
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde_json::Value;

use super::error::CaseSvcError;
use super::request_delivery::reconcile_delivery;

/// Every ask on `case_id`, oldest first, with `delivery` reconciled and `token_hash` stripped.
pub async fn case_request_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    case_id: &str,
) -> Result<Vec<Value>, CaseSvcError> {
    authorize_tool(principal, ws, "case.get").map_err(|_| CaseSvcError::Denied)?;

    let mut out = Vec::new();
    for mut request in lb_cases::requests_of_case(store, ws, case_id).await? {
        reconcile_delivery(store, ws, &mut request).await?;
        out.push(redacted(&request)?);
    }
    Ok(out)
}

/// The row as a drawer may see it: everything except the credential.
fn redacted(request: &CaseRequest) -> Result<Value, CaseSvcError> {
    let mut value = serde_json::to_value(request)
        .map_err(|e| CaseSvcError::Store(format!("request encode: {e}")))?;
    if let Some(obj) = value.as_object_mut() {
        obj.remove("token_hash");
    }
    Ok(value)
}
