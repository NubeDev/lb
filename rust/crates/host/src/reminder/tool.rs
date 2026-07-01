//! The MCP bridge for reminder verbs — host-native tools under the one MCP contract (README §6.5,
//! §3.7). The UI and other extensions reach the reminder surface the SAME way they reach any tool:
//! a qualified `reminder.<verb>` call with JSON in/out.
//!
//! The gate runs first (via each verb's own `authorize_reminder` — workspace-first, then
//! `mcp:reminder.<verb>:call`); this is what makes the mandatory deny-tests real. The bridged verbs
//! are the store/orchestration ones the UI drives directly: `create` / `update` / `delete` / `get`
//! / `list` (the scope's named MCP surface; live-feed + batch are explicit non-goals).

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_reminders::{Action, Reminder, ReminderError, ReminderStatus};
use serde_json::{json, Value};

use super::create::reminder_create;
use super::delete::reminder_delete;
use super::get::{reminder_get, reminder_list, StatusFilter};
use super::update::{reminder_update, ReminderPatch};
use crate::boot::Node;

/// Dispatch a `reminder.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb's own capability gate runs first (opaque `Denied`).
pub async fn call_reminder_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    let verb = qualified_tool
        .split_once('.')
        .map(|(_, v)| v)
        .unwrap_or(qualified_tool);

    let out = match verb {
        "create" => {
            let action = parse_action(input)?;
            let reminder = reminder_create(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                str_arg(input, "schedule")?,
                opt_u32(input, "max_runs")?,
                action,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(remind_to_tool)?;
            Ok(json!(reminder_json(&reminder)))
        }
        "update" => {
            let patch = ReminderPatch {
                schedule: opt_str(input, "schedule"),
                max_runs: opt_max_runs(input)?,
                enabled: opt_bool(input, "enabled"),
                action: opt_action(input)?,
            };
            let reminder = reminder_update(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                patch,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(remind_to_tool)?;
            Ok(json!(reminder_json(&reminder)))
        }
        "delete" => {
            reminder_delete(
                &node.store,
                principal,
                ws,
                str_arg(input, "id")?,
                u64_arg(input, "ts")?,
            )
            .await
            .map_err(remind_to_tool)?;
            Ok(json!({ "ok": true }))
        }
        "get" => {
            let reminder = reminder_get(&node.store, principal, ws, str_arg(input, "id")?)
                .await
                .map_err(remind_to_tool)?;
            Ok(json!({ "reminder": reminder.map(|r| reminder_json(&r)) }))
        }
        "list" => {
            let status = match input.get("status") {
                None | Some(Value::Null) => None,
                Some(v) => {
                    let s = v
                        .as_str()
                        .ok_or_else(|| ToolError::BadInput("status must be a string".into()))?;
                    Some(StatusFilter::parse(s).map_err(remind_to_tool)?)
                }
            };
            let limit = opt_u32(input, "limit")?.map(|n| n as usize);
            let reminders = reminder_list(&node.store, principal, ws, status, limit)
                .await
                .map_err(remind_to_tool)?;
            Ok(json!({ "reminders": reminders.iter().map(reminder_json).collect::<Vec<_>>() }))
        }
        _ => Err(ToolError::NotFound),
    }?;
    Ok(out)
}

/// The wire view of a reminder (camelCase for the UI). Mirrors the Rust record one-to-one.
fn reminder_json(r: &Reminder) -> Value {
    json!({
        "id": r.id,
        "schedule": r.schedule,
        "maxRuns": r.max_runs,
        "runs": r.runs,
        "enabled": r.enabled,
        "status": match r.status {
            ReminderStatus::Active => "active",
            ReminderStatus::Done => "done",
        },
        "action": action_json(&r.action),
        "principalSub": r.principal_sub,
        "nextAttemptTs": r.next_attempt_ts,
        "ts": r.ts,
    })
}

fn action_json(a: &Action) -> Value {
    match a {
        Action::ChannelPost { channel, body } => json!({
            "kind": "channel-post",
            "channel": channel,
            "body": body,
        }),
        Action::McpTool { tool, args } => json!({
            "kind": "mcp-tool",
            "tool": tool,
            "args": args,
        }),
        Action::Outbox {
            target,
            action,
            payload,
        } => json!({
            "kind": "outbox",
            "target": target,
            "action": action,
            "payload": payload,
        }),
    }
}

fn parse_action(input: &Value) -> Result<Action, ToolError> {
    let a = input
        .get("action")
        .ok_or_else(|| ToolError::BadInput("missing object arg: action".into()))?;
    action_from_value(a)
}

fn opt_action(input: &Value) -> Result<Option<Action>, ToolError> {
    match input.get("action") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => Ok(Some(action_from_value(v)?)),
    }
}

fn action_from_value(v: &Value) -> Result<Action, ToolError> {
    let kind = v
        .get("kind")
        .and_then(|k| k.as_str())
        .ok_or_else(|| ToolError::BadInput("action.kind missing".into()))?;
    Ok(match kind {
        "channel-post" => Action::ChannelPost {
            channel: str_arg(v, "channel")?.to_string(),
            body: v
                .get("body")
                .and_then(|b| b.as_str())
                .unwrap_or("")
                .to_string(),
        },
        "mcp-tool" => Action::McpTool {
            tool: str_arg(v, "tool")?.to_string(),
            args: v.get("args").cloned().unwrap_or(Value::Null),
        },
        "outbox" => Action::Outbox {
            target: str_arg(v, "target")?.to_string(),
            action: str_arg(v, "action")?.to_string(),
            payload: v
                .get("payload")
                .map(|p| match p {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default(),
        },
        other => return Err(ToolError::BadInput(format!("unknown action kind: {other}"))),
    })
}

fn opt_max_runs(input: &Value) -> Result<Option<Option<u32>>, ToolError> {
    match input.get("max_runs") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(v) => Ok(Some(Some(
            v.as_u64()
                .ok_or_else(|| ToolError::BadInput("max_runs must be a number".into()))?
                as u32,
        ))),
    }
}

fn remind_to_tool(e: ReminderError) -> ToolError {
    match e {
        ReminderError::Denied => ToolError::Denied,
        ReminderError::NotFound => ToolError::NotFound,
        ReminderError::BadCron(m) | ReminderError::BadInput(m) => ToolError::BadInput(m),
        ReminderError::Store(_) => ToolError::Extension(e.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}

fn opt_str(input: &Value, key: &str) -> Option<String> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn u64_arg(input: &Value, key: &str) -> Result<u64, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_u64())
        .ok_or_else(|| ToolError::BadInput(format!("missing u64 arg: {key}")))
}

fn opt_u32(input: &Value, key: &str) -> Result<Option<u32>, ToolError> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => Ok(Some(
            v.as_u64()
                .ok_or_else(|| ToolError::BadInput(format!("{key} must be a number").into()))?
                as u32,
        )),
    }
}

fn opt_bool(input: &Value, key: &str) -> Option<bool> {
    input.get(key).and_then(|v| v.as_bool())
}
