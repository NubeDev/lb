//! The MCP bridge for reminder verbs — host-native tools under the one MCP contract (README §6.5,
//! §3.7). The UI and other extensions reach the reminder surface the SAME way they reach any tool:
//! a qualified `reminder.<verb>` call with JSON in/out.
//!
//! The gate runs first (via each verb's own `authorize_reminder` — workspace-first, then
//! `mcp:reminder.<verb>:call`); this is what makes the mandatory deny-tests real. The bridged verbs
//! are the store/orchestration ones the UI drives directly: `create` / `update` / `delete` / `get`
//! / `list` (the scope's named MCP surface; live-feed + batch are explicit non-goals).

use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use lb_reminders::{Action, Reminder, ReminderError, ReminderStatus};
use serde_json::{json, Value};

use super::create::reminder_create;
use super::delete::reminder_delete;
use super::fire_now::reminder_fire;
use super::get::{reminder_get, reminder_list, StatusFilter};
use super::update::{reminder_update, ReminderPatch};
use crate::boot::Node;

/// Dispatch a `reminder.<verb>` MCP call. `input` is the verb's JSON arguments; the return is the
/// verb's JSON result. Each verb's own capability gate runs first (opaque `Denied`). Takes an
/// `&Arc<Node>` because `fire` reaches the shipped internal fire path (`fire_reminder`), which needs
/// the shared node (the CRUD verbs only touch `node.store`, reachable through the `Arc`).
pub async fn call_reminder_tool(
    node: &Arc<Node>,
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
            // Accept BOTH the nested wire form (`action:{kind,…}`, what the reminder engine + the
            // backward-compat tests pass) AND the descriptor's FLAT form (`action_kind` + per-kind
            // fields, what the generic palette posts straight from the form). See `create_action`.
            let action = create_action(input)?;
            let schedule = str_arg(input, "schedule")?;
            // `now`: the flat form omits `ts` (a generic "now" is not reminder knowledge, so the host
            // supplies it from its clock); the nested/test callers still pass an explicit `ts`.
            let now = opt_u64(input, "ts")?.unwrap_or_else(now_ts);
            // `id`: the flat form omits it (the UI no longer derives one) → derive a stable, ts-keyed id
            // server-side from the action; an explicit `id` (nested/test callers) is honored verbatim.
            let id = match opt_str(input, "id") {
                Some(id) => id,
                None => derive_reminder_id(&action, now),
            };
            let reminder = reminder_create(
                &node.store,
                principal,
                ws,
                &id,
                schedule,
                opt_u32(input, "max_runs")?,
                action,
                now,
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
                opt_u64(input, "ts")?.unwrap_or_else(now_ts),
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
                opt_u64(input, "ts")?.unwrap_or_else(now_ts),
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
        "fire" => {
            // Run-now: gated + idempotent, reusing the shipped internal fire path. `ts` is the logical
            // `now` the manual firing is keyed on (its own instant — see `fire_now`). Absent → the host
            // clock, so the GENERIC bridge path (a row control's `ts`-free `argsTemplate`) fires — the
            // reminder write verbs are tolerant of a missing `ts` exactly like `create`, rather than
            // forcing every caller (or the gateway) to inject a logical now. See
            // debugging/reminders/reminder-write-verbs-require-ts.md.
            let out = reminder_fire(
                node,
                principal,
                ws,
                str_arg(input, "id")?,
                opt_u64(input, "ts")?.unwrap_or_else(now_ts),
            )
            .await
            .map_err(remind_to_tool)?;
            Ok(out)
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

/// Build the create action from EITHER form: a nested `action:{kind,…}` object (the wire contract the
/// reminder engine + existing tests pass) OR the descriptor's FLAT form (`action_kind` + the per-kind
/// fields the palette form collects). Nested wins when present (backward compatible); otherwise the flat
/// keys are assembled. This is the seam that lets the generic frontend post the declared form directly —
/// it never reshapes args into a nested `action` (zero reminder-specific knowledge in the UI).
fn create_action(input: &Value) -> Result<Action, ToolError> {
    match input.get("action") {
        Some(a) if !a.is_null() => action_from_value(a),
        _ => action_from_flat(input),
    }
}

/// Assemble an [`Action`] from the descriptor's FLAT form fields: `action_kind` ∈ {channel-post,
/// mcp-tool, outbox} selects the variant; the per-kind fields are read at the top level —
///   - `channel-post` → `channel`, `body`;
///   - `mcp-tool`     → `tool`, `args` (a JSON STRING is parsed to a `Value`, else passed through/Null);
///   - `outbox`       → `target`, `action_action` (the outbox action verb, renamed in the flat form to
///                      avoid colliding with the nested `action` key), `payload`.
fn action_from_flat(input: &Value) -> Result<Action, ToolError> {
    let kind = input
        .get("action_kind")
        .and_then(|k| k.as_str())
        .ok_or_else(|| ToolError::BadInput("missing string arg: action_kind".into()))?;
    Ok(match kind {
        "channel-post" => Action::ChannelPost {
            channel: str_arg(input, "channel")?.to_string(),
            body: opt_str(input, "body").unwrap_or_default(),
        },
        "mcp-tool" => Action::McpTool {
            tool: str_arg(input, "tool")?.to_string(),
            args: flat_args(input.get("args")),
        },
        "outbox" => Action::Outbox {
            target: str_arg(input, "target")?.to_string(),
            action: str_arg(input, "action_action")?.to_string(),
            payload: opt_str(input, "payload").unwrap_or_default(),
        },
        other => return Err(ToolError::BadInput(format!("unknown action_kind: {other}"))),
    })
}

/// Coerce the flat-form `args` into the `McpTool` action's `Value`: a JSON STRING is parsed (the form
/// collects args as text) — a non-JSON string is kept as a string; a non-string value is passed through;
/// absent → `Null`.
fn flat_args(v: Option<&Value>) -> Value {
    match v {
        None | Some(Value::Null) => Value::Null,
        Some(Value::String(s)) => serde_json::from_str(s).unwrap_or(Value::String(s.clone())),
        Some(other) => other.clone(),
    }
}

/// Derive a stable, deterministic-friendly reminder id when the flat form omits one. Built from the
/// action's identifying field (channel / tool / target) slugged + the injected `now` — no random uuid,
/// so a given (action, now) yields the same id (idempotent re-create upserts, mirroring the verb's
/// `id`-idempotency). The `ts` is host-injected, so the id is stable within a call.
fn derive_reminder_id(action: &Action, now: u64) -> String {
    let stem = match action {
        Action::ChannelPost { channel, .. } => format!("post-{channel}"),
        Action::McpTool { tool, .. } => format!("tool-{tool}"),
        Action::Outbox { target, .. } => format!("outbox-{target}"),
    };
    let slug: String = stem
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    format!("reminder-{slug}-{now}")
}

/// The host's wall-clock `now`, in logical SECONDS (the reminder crate's `ts` unit — `next_after`
/// does cron math in seconds; testing §3 keeps the crate itself clock-free). Supplied when the flat
/// form omits `ts` (a generic "now" is not reminder knowledge). Used ONCE at this boundary.
fn now_ts() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
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

fn opt_u64(input: &Value, key: &str) -> Result<Option<u64>, ToolError> {
    match input.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => Ok(Some(v.as_u64().ok_or_else(|| {
            ToolError::BadInput(format!("{key} must be a number"))
        })?)),
    }
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
