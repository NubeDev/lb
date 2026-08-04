//! `update.credential.set {value}` and `update.credential.claim {code?}` — the two ways an operator
//! puts a credential on this node. Both ride the ONE `mcp:update.credential:call` grant, which is
//! documented as **equivalent to backend admin**: lb cannot narrow a backend's credential, so the
//! honest move is to make the grant's weight visible rather than quietly bundle it.
//!
//! Neither verb ever returns, logs, or echoes the value — only a fingerprint. `set` **verifies
//! before sealing** (scope decision 4): a store write that has not been proven to work is a trap set
//! for the next outage, so a mistyped token fails at enrolment instead of at 3am during an update.

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::Value;

use super::audit;
use super::context::installed;
use super::credential::{fingerprint, seal};
use super::error::UpdateError;
use super::model::{CredentialSource, CredentialStatus};
use crate::boot::Node;

/// The capability both enrolment verbs ride. Kept out of every default role bundle beyond
/// workspace-admin, and out of the default agent ceiling.
pub const CREDENTIAL_CAP: &str = "update.credential";

/// `update.credential.set {value}` — verify the candidate against the backend, then seal it
/// host-owned in the boot workspace. Returns `{configured, source, fingerprint}`.
pub async fn set(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, CREDENTIAL_CAP)?;
    let value = input
        .get("value")
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .ok_or_else(|| ToolError::BadInput("missing/!string arg: value".into()))?;
    let inst = installed(node)?;
    // VERIFY FIRST. A failure here leaves the store untouched — nothing was written to roll back.
    inst.cfg.provider.verify_credential(value).await?;
    seal(node, &inst, value).await?;
    audit::record(
        &node.store,
        &inst.boot_workspace,
        principal.sub(),
        "update.credential.set",
        "credential",
        "sealed",
        None,
    )
    .await;
    Ok(sealed_status(value))
}

/// `update.credential.claim {code?}` — drive the backend's own one-time enrolment handshake. The
/// provider returns the plaintext **to lb, not to the caller**; lb seals it and answers with a
/// fingerprint. A backend needing a second factor answers `Unauthorized{code_required: true}` and
/// the UI re-submits with `{code}`.
pub async fn claim(
    node: &Node,
    principal: &Principal,
    ws: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    authorize_tool(principal, ws, CREDENTIAL_CAP)?;
    let code = input.get("code").and_then(Value::as_str);
    let inst = installed(node)?;
    let value = inst.cfg.provider.provision_credential(code).await?;
    if value.is_empty() {
        return Err(UpdateError::Backend("the backend returned an empty credential".into()).into());
    }
    seal(node, &inst, &value).await?;
    audit::record(
        &node.store,
        &inst.boot_workspace,
        principal.sub(),
        "update.credential.claim",
        "credential",
        "sealed",
        None,
    )
    .await;
    Ok(sealed_status(&value))
}

/// The reply both verbs share: configured, sealed, fingerprint. Never the value.
fn sealed_status(value: &str) -> Value {
    serde_json::to_value(CredentialStatus {
        configured: true,
        source: CredentialSource::Sealed,
        fingerprint: Some(fingerprint(value)),
    })
    .unwrap_or(Value::Null)
}
