//! The read half of the family — `update.status`, `update.check`, `update.history`,
//! `update.credential.status`. All four gate on the ONE collapsed grant `mcp:update.read:call`:
//! reading a version is not applying one, so the three grants split by blast radius, not by verb.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use super::context::{installed, prepare};
use super::model::UpdateStatus;
use crate::boot::Node;

/// The capability all four read verbs ride (see `tool_gate::gate_tool_for` for the alias table that
/// makes the collapse expressible — a per-verb registry cannot express it, scope decision 7).
pub const READ_CAP: &str = "update.read";

/// `update.status` — the whole node-update picture in one read, including **key durability** (a
/// per-boot signing key means this update signs the operator out mid-flight, and a UI cannot warn
/// about that unless the node says so).
///
/// On a node with no provider this is NOT an error: it answers `{"supported": false}` — the honest
/// `UnconfiguredModel` posture. Every other verb is a clean `Unsupported`.
pub async fn status(node: &Node, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, READ_CAP)?;
    let Ok(inst) = installed(node) else {
        return Ok(to_value(UpdateStatus::unsupported()));
    };
    let p = prepare(node, principal, inst).await?;
    let mut st = p.inst.cfg.provider.status(&p.cx).await?;
    // lb's custody view WINS over the provider's: a provider cannot report a credential state lb did
    // not resolve, and it has no way to observe the sealed record in the first place.
    st.credential = p.resolved.status();
    Ok(to_value(st))
}

/// `update.check` — the reachable versions **in the provider's order**. lb does not parse, compare
/// or order version strings (§Risks); `newest` is simply the provider's first row, and
/// `update_available` is "there is a first row and it is not what we are running".
pub async fn check(node: &Node, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, READ_CAP)?;
    let inst = installed(node)?;
    let p = prepare(node, principal, inst).await?;
    let available = p.inst.cfg.provider.check(&p.cx).await?;
    let current = p
        .inst
        .cfg
        .provider
        .status(&p.cx)
        .await
        .ok()
        .and_then(|s| s.current_version);
    let newest = available.first().map(|v| v.version.clone());
    let update_available = matches!((&newest, &current), (Some(n), c) if Some(n) != c.as_ref());
    Ok(json!({
        "current": current,
        "newest": newest,
        "update_available": update_available,
        "available": available,
    }))
}

/// `update.history {limit?}` — the provider's journal. The executor's journal is the authority on
/// what happened to the binary; lb's audit records who asked (scope decision 2).
pub async fn history(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, READ_CAP)?;
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .min(MAX_HISTORY_LIMIT) as u32;
    let inst = installed(node)?;
    let p = prepare(node, principal, inst).await?;
    let events = p.inst.cfg.provider.history(&p.cx, limit).await?;
    Ok(json!({ "events": events }))
}

/// The default and hard ceiling on `history {limit}` — bounded like every other list read, so a
/// caller cannot ask a backend for an unbounded journal.
const DEFAULT_HISTORY_LIMIT: u64 = 20;
const MAX_HISTORY_LIMIT: u64 = 200;

/// `update.credential.status` — `{configured, source, fingerprint}`, **never the value**. Reachable
/// on an unconfigured node too, where it answers the honest not-configured shape rather than erroring:
/// "is this node enrolled?" is a question a UI must be able to ask before it knows the answer.
pub async fn credential_status(
    node: &Node,
    principal: &Principal,
    ws: &str,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, READ_CAP)?;
    let Ok(inst) = installed(node) else {
        return Ok(to_value(super::model::CredentialStatus::default()));
    };
    let p = prepare(node, principal, inst).await?;
    Ok(to_value(p.resolved.status()))
}

/// Serialize a pinned model type. The shapes are plain data with infallible `Serialize` impls, so a
/// failure here is a programming error, not a runtime condition — `Null` keeps it out of the
/// error channel rather than inventing one.
fn to_value<T: serde::Serialize>(v: T) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}
