//! `update.apply {version}` and `update.rollback` — the two verbs that ask the backend to replace
//! this process. Both ride the ONE `mcp:update.apply:call` grant: applying a version and rolling one
//! back are the same blast radius, and neither is the same as holding the backend's credential.
//!
//! **They return accepted, never done.** The process serving the reply is the process about to be
//! replaced; any other contract is a lie the first time it is true. The verdict is read back through
//! `update.status` after the node returns, which is also why `history` is a verb and not a field.
//!
//! lb writes exactly one audit row per call, BEFORE the provider is told to go: "who replaced the
//! binary on this box" must survive the binary, including when the swap kills the replier mid-reply.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::{json, Value};

use super::audit;
use super::context::{installed, prepare};
use crate::boot::Node;

/// The capability both write verbs ride. Never granted to a member, and never in the default agent
/// capability ceiling — an agent that can replace the node's binary is a different product.
pub const APPLY_CAP: &str = "update.apply";

/// `update.apply {version}` — accept an update to `version`.
pub async fn apply(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, APPLY_CAP)?;
    let version = input
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| ToolError::BadInput("missing/!string arg: version".into()))?;
    let inst = installed(node)?;
    let p = prepare(node, principal, inst).await?;
    // Audited BEFORE the call: an accepted apply may never come back to write it afterwards.
    audit::record(
        &node.store,
        &p.inst.boot_workspace,
        principal.sub(),
        "update.apply",
        version,
        "requested",
        None,
    )
    .await;
    let accepted = p.inst.cfg.provider.apply(&p.cx, version).await?;
    audit::record(
        &node.store,
        &p.inst.boot_workspace,
        principal.sub(),
        "update.apply",
        version,
        "accepted",
        Some(&accepted.tx),
    )
    .await;
    Ok(json!({ "accepted": true, "tx": accepted.tx }))
}

/// `update.rollback` — accept a rollback to whatever the backend considers the previous good state.
/// lb names no version here on purpose: which state "previous" is belongs to the executor's journal.
pub async fn rollback(node: &Node, principal: &Principal, ws: &str) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, APPLY_CAP)?;
    let inst = installed(node)?;
    let p = prepare(node, principal, inst).await?;
    audit::record(
        &node.store,
        &p.inst.boot_workspace,
        principal.sub(),
        "update.rollback",
        "previous",
        "requested",
        None,
    )
    .await;
    let accepted = p.inst.cfg.provider.rollback(&p.cx).await?;
    audit::record(
        &node.store,
        &p.inst.boot_workspace,
        principal.sub(),
        "update.rollback",
        "previous",
        "accepted",
        Some(&accepted.tx),
    )
    .await;
    Ok(json!({ "accepted": true, "tx": accepted.tx }))
}
