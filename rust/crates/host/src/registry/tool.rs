//! The MCP bridge for the registry's **read** verbs — host-native tools under the one MCP contract
//! (README §6.4 "the registry is itself a platform extension exposing install/list/update as MCP
//! tools", §6.5). The UI and other extensions reach the catalog the SAME way they reach any tool: a
//! qualified `registry.<verb>` call with JSON in/out.
//!
//! The MCP gate runs first (`authorize_registry` — workspace-first, then `mcp:registry.<verb>:call`),
//! so a ws-B caller or one without the grant is refused HERE, before the verb runs — what makes the
//! mandatory MCP-surface isolation + deny tests real.
//!
//! `pull` and `install` are NOT bridged here: they need the `Source` seam + the publisher-key
//! allow-list (environment, not JSON args) — exactly like `workflow::triage` needs a `ModelAccess` and
//! so has its own typed entry (`install_from_registry`). The bridged verbs are the store-only catalog
//! reads the UI drives directly: `list` and `resolve`.

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use super::authorize::authorize_registry;
use super::catalog::{list_catalog, resolve};
use super::error::RegistryServiceError;
use crate::boot::Node;

/// Dispatch a `registry.<verb>` MCP call (the read verbs). `input` is the verb's JSON arguments; the
/// return is the verb's JSON result. The MCP gate runs first.
pub async fn call_registry_tool(
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

    // Gate: the MCP surface — workspace-first, then mcp:registry.<verb>:call. Opaque on denial.
    authorize_registry(principal, ws, verb).map_err(rs_to_tool)?;

    let out = match verb {
        "list" => {
            let entries = list_catalog(&node.store, ws, str_arg(input, "ext_id")?)
                .await
                .map_err(|e| ToolError::Extension(e.to_string()))?;
            json!({ "entries": entries })
        }
        "resolve" => {
            let entry = resolve(
                &node.store,
                ws,
                str_arg(input, "ext_id")?,
                str_arg(input, "version")?,
            )
            .await
            .map_err(|e| ToolError::Extension(e.to_string()))?;
            json!({ "entry": entry })
        }
        _ => return Err(ToolError::NotFound),
    };
    Ok(out)
}

/// Map the registry service error onto the MCP tool error. `Denied` stays opaque; `Unverified` and
/// `NotAvailable` surface as distinguishable client errors (a bad/absent artifact is not a hidden
/// resource — the caller asked for something specific).
fn rs_to_tool(e: RegistryServiceError) -> ToolError {
    match e {
        RegistryServiceError::Denied => ToolError::Denied,
        RegistryServiceError::Unverified => {
            ToolError::BadInput("artifact failed verification".into())
        }
        RegistryServiceError::NotAvailable(m) => ToolError::BadInput(format!("not available: {m}")),
        RegistryServiceError::Store(s) => ToolError::Extension(s.to_string()),
        RegistryServiceError::Load(l) => ToolError::Extension(l.to_string()),
    }
}

fn str_arg<'a>(input: &'a Value, key: &str) -> Result<&'a str, ToolError> {
    input
        .get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::BadInput(format!("missing string arg: {key}")))
}
