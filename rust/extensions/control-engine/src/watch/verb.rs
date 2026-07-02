//! The `control-engine.watch` verb (slice-6): gate → resolve appliance → derive the series → arm the
//! live COV pump → return what S7 opens. Self-checks `mcp:control-engine.watch:call` FIRST (the inbound
//! `native.call` carries no caller identity — same reason the S4 registry verbs self-check).
//!
//! Returns `{ series, subject }`:
//!   - `series` — the deterministic series name; S7 opens `GET /series/{series}/stream` on the gateway.
//!   - `subject` — the workspace bus subject the motion rides (`ws/{ws}/series/{series}`), for a caller
//!     that subscribes the bus directly (the two-node test asserts on this; the SSE relays it verbatim).
//!
//! Arming is idempotent per `(appliance, scope)`: two watches for the same target share ONE pump
//! (refcount), and the last release (or `appliance.remove`) tears it down (see `super::WatchRegistry`).

use serde_json::{json, Value};

use crate::engine::Registry as ClientRegistry;
use crate::host::{HostCtx, HostError};
use crate::resolve;

use super::series::target;
use super::WatchRegistry;

/// Run `control-engine.watch { appliance, scope? }`. Resolves the appliance to a local CE base (S4),
/// binds the client, arms the pump, and returns `{ series, subject }`.
pub async fn run(
    host: &HostCtx,
    clients: &ClientRegistry,
    watches: &WatchRegistry,
    input: &Value,
) -> Result<Value, HostError> {
    host.require("control-engine.watch")?;

    let selector = input
        .get("appliance")
        .and_then(Value::as_str)
        .unwrap_or_default();
    // Resolve the appliance in THIS workspace — an unknown/other-ws selector is a clean not-found (the
    // isolation wall), so a ws-B caller cannot watch ws-A's appliance.
    let resolved = resolve::resolve(host, selector).await?;
    let bound = clients
        .get(&resolved.base)
        .map_err(HostError::BadResponse)?;

    let tgt = target(selector, input);
    let subject = format!("ws/{}/series/{}", host.ws(), tgt.series);

    watches.arm(
        host.clone(),
        bound.engine.clone(),
        selector,
        &tgt.series,
        tgt.scope,
    );

    Ok(json!({ "series": tgt.series, "subject": subject }))
}
