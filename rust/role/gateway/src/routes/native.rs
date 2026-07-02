//! `POST /native/call` — the browser's bridge to a **native-tier sidecar's own tools** (native-tier
//! scope, README §6.5: the supervisor control plane is reached as MCP tools under the one contract).
//! It is the native-tier peer of `POST /mcp/call`: `/mcp/call` dispatches host-native (`series.*`) and
//! wasm `<ext>.<tool>` verbs, but a sidecar's tools (`ros.list`, `point.write`, …) go through
//! `native.call`, which is a `Launcher`-typed entry (`call_sidecar` needs the OS launcher for its
//! restart-on-demand crash path) and so was not reachable through the string dispatcher. This route
//! wires it, so a federated page can drive its extension's sidecar the SAME way it drives any tool.
//!
//! The workspace + principal come from the **verified session token** (§7), never the body — so a page
//! is exactly as denied as a forged call. The host gate (`mcp:native.call:call`, workspace-first) runs
//! inside `call_sidecar`; the sidecar independently self-checks the fine-grained per-verb cap against
//! its own `LB_EXT_TOKEN` grant (native-tier: the control line carries no caller identity). Defense in
//! depth, no change to that model.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{call_sidecar, OsLauncher};
use serde::Deserialize;
use serde_json::Value;

use crate::session::authenticate;
use crate::state::Gateway;

/// The bridge request: which sidecar (`ext_id`), which of its tools, and the tool's JSON args. No
/// token, no workspace — both come from the verified session, not the page (the hard wall §7).
#[derive(Debug, Deserialize)]
pub struct NativeCall {
    pub ext_id: String,
    pub tool: String,
    /// The tool's args. A JSON value (stringified for the control line) or omitted (`{}`).
    #[serde(default)]
    pub input: Value,
}

/// Forward one bridged native-sidecar tool call. `401` if the session token is missing/bad; `403` if
/// the verified principal lacks `mcp:native.call:call`, the sidecar is not running, or the child
/// faulted (opaque — no existence oracle); the tool's JSON output otherwise.
pub async fn native_call(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Json(body): Json<NativeCall>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let principal = authenticate(&gw, &headers)
        .await
        .map_err(|e| e.into_response())?;
    // Serialize the args for the control line (a native sidecar receives a JSON string). An absent/
    // null input is an empty object, matching `/mcp/call`.
    let input = if body.input.is_null() {
        "{}".to_string()
    } else {
        body.input.to_string()
    };
    // `OsLauncher` is the production launcher (a unit struct), matching `node/src/federation.rs`; it is
    // only used if the child died and must be restarted-on-demand before the retry.
    let out = call_sidecar(
        &gw.node,
        &OsLauncher,
        &principal,
        principal.ws(),
        &body.ext_id,
        &body.tool,
        &input,
        gw.now(),
    )
    .await
    .map_err(|e| (StatusCode::FORBIDDEN, e.to_string()))?;
    let value: Value = serde_json::from_str(&out).unwrap_or(Value::String(out));
    Ok(Json(value))
}
