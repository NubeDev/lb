//! `secret.*` host MCP tools (secrets scope) — the CRUD surface extensions/agents/UI reach over
//! the host-callback ABI, so the same three-gate read runs whether a value is fetched by an
//! extension's own process, an agent, or the UI. Each tool maps to one crate verb; the crate does
//! gates 1+2 (workspace, cap) and gate 3 (owner/visibility).
//!
//! The mediation invariant holds across the surface: `secret.list` returns **metadata only**
//! (`path`/`owner`/`visibility`), and only `secret.get` to an authorized principal ever returns
//! the value. The owner is the host-stamped principal (`caller ∩ install-grant`), never a guest
//! claim — the crate stamps `owner` from `principal.sub()`.
//!
//! One responsibility per file (secrets scope §"How it fits the core"): each verb is its own MCP
//! tool backed by its own capability, dispatched from [`call_secret_tool`].

mod del;
mod descriptors;
mod get;
mod list;
mod set;
mod set_visibility;

use del::secret_delete;
pub(crate) use descriptors::secret_descriptors;
use get::secret_get;
use list::secret_list;
use set::secret_set;
use set_visibility::secret_set_visibility;

use lb_auth::Principal;
use lb_mcp::{authorize_tool, ToolError};
use serde_json::Value;

use crate::boot::Node;

/// Parse a required non-empty string arg, or return [`ToolError::BadInput`].
pub(crate) fn req_str(input: &Value, key: &str) -> Result<String, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| ToolError::BadInput(format!("missing arg: {key}")))
}

/// Map a [`lb_secrets::SecretsError`] to the opaque MCP error. `Denied` carries no detail (a
/// denied caller learns nothing); the value never reaches the error string (mediation invariant).
pub(crate) fn map_err(e: lb_secrets::SecretsError) -> ToolError {
    use lb_secrets::SecretsError;
    match e {
        SecretsError::Denied => ToolError::Denied,
        SecretsError::NotFound => ToolError::BadInput("not found".into()),
        SecretsError::Store(s) => ToolError::Extension(s.to_string()),
    }
}

/// Dispatch a `secret.*` MCP call (secrets scope, README §6.5: the secret surface is reached as
/// MCP tools under the one contract). The UI, rules, the AI agent, and other extensions manage
/// secrets the SAME way they reach any tool — a qualified call. The MCP `mcp:<tool>:call` gate
/// runs here first (opaque `Denied`); each verb then re-runs the `secret:<path>:*` capability +
/// the gate-3 owner/visibility wall inside `lb-secrets`.
pub async fn call_secret_tool(
    node: &Node,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    // The MCP gate first — a denied caller learns nothing about existence (mcp scope).
    authorize_tool(principal, ws, qualified_tool).map_err(|_| ToolError::Denied)?;
    match qualified_tool {
        "secret.set" => secret_set(node, principal, ws, input).await,
        "secret.get" => secret_get(node, principal, ws, input).await,
        "secret.set_visibility" => secret_set_visibility(node, principal, ws, input).await,
        "secret.delete" => secret_delete(node, principal, ws, input).await,
        "secret.list" => secret_list(node, principal, ws, input).await,
        _ => Err(ToolError::NotFound),
    }
}
