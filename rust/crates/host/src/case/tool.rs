//! The MCP bridge for case verbs — host-native tools under the one MCP contract (case-plane scope).
//! UI, agents and extensions reach `case.*` the SAME way they reach any wasm tool: a qualified call
//! with JSON in/out. The MCP gate runs inside each verb FIRST (workspace-first §7, then the verb's
//! capability), so a ws-B caller or one without the grant is refused before the verb runs.
//!
//! Modelled on `insight/tool.rs`. The `ts`-taking verbs take their logical `now` from the args (the
//! caller's clock — determinism §3) and fall back to the host wall clock through
//! [`super::clock::normalize_ts`], which also rescues a door that stamped epoch SECONDS.
//!
//! **Four verbs here gate on a cap that is not their name** (`case.members`/`case.events` on
//! `case.get`; `case.merge`/`case.split` on `case.open`; `case.assign`/`case.snooze`/`case.comment`
//! on `case.workflow`). Each of those needs an arm in `tool_gate.rs` — the outer gate consults that
//! table, and a missing arm demands a capability that exists in no role bundle, which is `Denied`
//! for every caller including admins. Only a POSITIVE test catches it.

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::breach::case_breach;
use super::error::CaseSvcError;
use super::{
    case_assign, case_comment, case_events, case_get, case_list, case_members, case_merge,
    case_open, case_party_list, case_party_upsert, case_policy_sla_list, case_policy_sla_set,
    case_request_list, case_request_nudge, case_request_reply, case_request_send,
    case_request_view, case_request_withdraw, case_snooze, case_split, case_workflow,
    rule_scorecard,
};
use crate::boot::Node;

/// Dispatch a `case.<verb>` MCP call. The outer `is_host_native` gate already ran the aliased
/// capability; each verb here re-runs it inside (defense in depth).
pub async fn call_case_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let store = &node.store;
    let ts = super::clock::normalize_ts(input.get("ts").and_then(Value::as_u64).unwrap_or(0));
    match qualified_tool {
        // The SLA policy plane (case-plane scope). ADMIN, and dispatched by EXACT name from
        // `HOST_NATIVE_EXACT` — the host does not reserve the whole `policy.` namespace against a
        // hypothetical extension whose id is `policy`, the same reasoning `ext.list` and `update.*`
        // already carry. Both verbs gate on their OWN name, so neither needs a `tool_gate.rs` arm.
        "policy.sla.set" => {
            let policy: lb_cases::ServicePolicy = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("policy.sla.set: {e}")))?;
            case_policy_sla_set(store, principal, ws, &policy)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "id": policy.id }))
        }
        "policy.sla.list" => {
            let policies = case_policy_sla_list(store, principal, ws)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(policies).unwrap_or(Value::Null))
        }
        // case-plane scope §7: the detector feedback loop. VIEWER, read-only, and dispatched by
        // EXACT name — see `HOST_NATIVE_EXACT`. It gates on its OWN name, so no `tool_gate.rs` arm.
        "rule.scorecard" => {
            let rows = rule_scorecard(
                store,
                principal,
                ws,
                input.get("rule_ref").and_then(Value::as_str),
                input.get("site").and_then(Value::as_str),
                input
                    .get("since")
                    .and_then(Value::as_u64)
                    .map(super::clock::normalize_ts),
                input
                    .get("until")
                    .and_then(Value::as_u64)
                    .map(super::clock::normalize_ts),
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(json!({ "rows": rows }))
        }
        // The party roster (case-plane scope, wave 2). ADMIN, dispatched by EXACT name from
        // `HOST_NATIVE_EXACT` — `party` is a plausible extension id, so the host reserves the two
        // verbs it owns and not the namespace (the `policy.sla.*` reasoning, unchanged).
        "party.upsert" => {
            // Decoded through `PartyInput`, which DENIES unknown fields: a misplaced key (a
            // top-level `email` that belongs under `contact`) is a loud refusal here rather than a
            // successful write of a party nobody can reach. The stored record stays permissive —
            // see `party_upsert.rs`.
            let input: super::PartyInput = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("party.upsert: {e}")))?;
            let party: lb_cases::Party = input.into();
            case_party_upsert(store, principal, ws, &party)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "id": party.id }))
        }
        "party.list" => {
            let kind = opt_enum_arg(input, "kind")?;
            let parties = case_party_list(
                store,
                principal,
                ws,
                kind,
                input.get("site").and_then(Value::as_str),
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(parties).unwrap_or(Value::Null))
        }
        // The external-party round trip. `send`/`withdraw`/`nudge` ride `mcp:case.request.send:call`
        // (the last two through `tool_gate.rs` aliases); `list` rides `case.get`; `view`/`reply` are
        // the token principal's two verbs and gate on their own caps, which exist in no bundle.
        "case.request.send" => {
            let brief = match input.get("brief") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    serde_json::from_value(v.clone())
                        .map_err(|e| ToolError::BadInput(format!("arg `brief`: {e}")))?,
                ),
            };
            let request = case_request_send(
                node,
                principal,
                ws,
                str_arg(input, "case_id")?,
                str_arg(input, "party_id")?,
                enum_arg(input, "ask")?,
                brief,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            // The raw token is deliberately absent from this reply: it belongs in the mail, and a
            // verb that returned it would put a working link in every caller's response log.
            Ok(serde_json::to_value(request).unwrap_or(Value::Null))
        }
        "case.request.withdraw" => {
            let request = case_request_withdraw(store, principal, ws, str_arg(input, "id")?, ts)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(request).unwrap_or(Value::Null))
        }
        "case.request.nudge" => {
            let fired = case_request_nudge(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "stage")?,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(json!({ "fired": fired }))
        }
        "case.request.list" => {
            let requests = case_request_list(store, principal, ws, str_arg(input, "case_id")?)
                .await
                .map_err(svc_to_tool)?;
            Ok(Value::Array(requests))
        }
        "case.request.view" => {
            let view = case_request_view(store, principal, ws, str_arg(input, "id")?, ts)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(view).unwrap_or(Value::Null))
        }
        "case.request.reply" => {
            let reply: lb_cases::Reply =
                serde_json::from_value(input.get("reply").cloned().unwrap_or(Value::Null))
                    .map_err(|e| ToolError::BadInput(format!("arg `reply`: {e}")))?;
            let receipt =
                case_request_reply(store, principal, ws, str_arg(input, "id")?, reply, ts)
                    .await
                    .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(receipt).unwrap_or(Value::Null))
        }
        "case.get" => {
            let case = case_get(store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(case).unwrap_or(Value::Null))
        }
        "case.list" => {
            let mut query: lb_cases::ListQuery = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(format!("case.list query: {e}")))?;
            // The `snoozed` axis is relative to a clock; a caller that omitted one gets the host's,
            // so "is this parked right now" is answered against real time rather than the epoch.
            if query.now == 0 {
                query.now = ts;
            }
            let page = case_list(store, principal, ws, query)
                .await
                .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(page).unwrap_or(Value::Null))
        }
        "case.members" => {
            let page = case_members(
                store,
                principal,
                ws,
                str_arg(input, "case_id")?,
                input.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize,
                input.get("after").and_then(Value::as_str),
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(page).unwrap_or(Value::Null))
        }
        "case.events" => {
            let page = case_events(
                store,
                principal,
                ws,
                str_arg(input, "case_id")?,
                input.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize,
                input.get("after").and_then(Value::as_u64),
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(page).unwrap_or(Value::Null))
        }
        "case.open" => {
            let also = string_array(input, "also")?;
            let case = case_open(
                node,
                principal,
                ws,
                str_arg(input, "title")?,
                str_arg(input, "primary_insight")?,
                &also,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(case).unwrap_or(Value::Null))
        }
        "case.merge" => {
            let closed = case_merge(
                node,
                principal,
                ws,
                str_arg(input, "from")?,
                str_arg(input, "into")?,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(closed).unwrap_or(Value::Null))
        }
        "case.split" => {
            let ids = string_array(input, "insight_ids")?;
            let case = case_split(
                node,
                principal,
                ws,
                str_arg(input, "from")?,
                &ids,
                str_arg(input, "title")?,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(case).unwrap_or(Value::Null))
        }
        "case.workflow" => {
            let workflow = enum_arg(input, "workflow")?;
            // `resolution` is deliberately NOT defaulted or inferred: the crate refuses `resolved`
            // without one, and guessing here would be a second door round the invariant.
            let resolution = opt_enum_arg(input, "resolution")?;
            let waiting_on = opt_enum_arg(input, "waiting_on")?;
            let case = case_workflow(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                workflow,
                resolution,
                waiting_on,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(case).unwrap_or(Value::Null))
        }
        "case.assign" => {
            // `assignee: null` CLEARS (un-assign); an absent key is the same gesture — there is
            // nothing else "assign with no assignee" could mean.
            let assignee = input.get("assignee").and_then(Value::as_str);
            let outcome = case_assign(node, principal, ws, str_arg(input, "id")?, assignee, ts)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "assigned_to": outcome.assigned_to, "changed": outcome.changed }))
        }
        "case.snooze" => {
            let until = input
                .get("until")
                .and_then(Value::as_u64)
                .ok_or_else(|| ToolError::BadInput("missing or non-u64 arg: until".into()))?;
            let case = case_snooze(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                super::clock::normalize_ts(until),
                input.get("reason").and_then(Value::as_str),
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(serde_json::to_value(case).unwrap_or(Value::Null))
        }
        "case.comment" => {
            let seq = case_comment(
                store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "text")?,
                ts,
            )
            .await
            .map_err(svc_to_tool)?;
            Ok(json!({ "seq": seq }))
        }
        // The breach alarm's own door (case-plane scope, sla-clock). Gates on its OWN name, so it
        // needs no `tool_gate.rs` arm; the cap is in no role bundle and is held only by the SLA
        // clock's subject (`super::breach_reminder`). A person does not declare a breach.
        "case.breach" => {
            let case = case_breach(node, principal, ws, str_arg(input, "case_id")?, ts)
                .await
                .map_err(svc_to_tool)?;
            Ok(json!({ "breached": case.is_some(), "case": case }))
        }
        _ => Err(ToolError::NotFound),
    }
}

/// Map the service error onto the MCP tool error (denials opaque).
fn svc_to_tool(e: CaseSvcError) -> ToolError {
    match e {
        CaseSvcError::Denied => ToolError::Denied,
        CaseSvcError::BadInput(m) => ToolError::BadInput(m),
        CaseSvcError::Store(s) => ToolError::Extension(s),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput(format!("missing or non-string arg: {key}")))
}

fn string_array(input: &Value, key: &str) -> Result<Vec<String>, ToolError> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(arr)) => arr
            .iter()
            .map(|v| {
                v.as_str().map(str::to_string).ok_or_else(|| {
                    ToolError::BadInput(format!("every entry in `{key}` must be a string"))
                })
            })
            .collect(),
        Some(_) => Err(ToolError::BadInput(format!("`{key}` must be an array"))),
    }
}

/// Decode a required closed-set argument through serde, so the accepted spellings can never drift
/// from the enum's own wire form.
fn enum_arg<T: serde::de::DeserializeOwned>(input: &Value, key: &str) -> Result<T, ToolError> {
    let value = input
        .get(key)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))?;
    serde_json::from_value(value.clone())
        .map_err(|e| ToolError::BadInput(format!("arg `{key}`: {e}")))
}

fn opt_enum_arg<T: serde::de::DeserializeOwned>(
    input: &Value,
    key: &str,
) -> Result<Option<T>, ToolError> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone())
            .map(Some)
            .map_err(|e| ToolError::BadInput(format!("arg `{key}`: {e}"))),
    }
}
