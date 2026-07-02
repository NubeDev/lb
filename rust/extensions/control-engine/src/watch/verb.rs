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
use crate::tools::raw_tree;

use super::series::target;
use super::{scope_uids, WatchRegistry};

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

    // Resolve the subscription scope. The engine only pushes COV frames for EXPLICITLY
    // subscribed components — an empty subscribe streams zero value frames (verified on the
    // live engine). So "no explicit scope" (the UI's default = "watch the whole appliance")
    // must be expanded to every component UID in the tree BEFORE arming, or the pump carries
    // nothing and the canvas shows no live values. See
    // `docs/debugging/frontend/ce-canvas-empty-cov-scope-no-live-values.md`.
    let input = expand_scope(&resolved.base, input).await;
    let tgt = target(selector, &input);
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

/// Expand an EMPTY component scope into the appliance's full UID set. If the caller already
/// gave `scope.components` (or `scope.properties`) we honour it verbatim — an explicit
/// scope is a deliberate narrowing. Only the "whole appliance" default (no components AND no
/// properties) is expanded: we fetch the tolerant raw tree (`tools::raw_tree`) and inject
/// every component UID as `scope.components`, so the pump's `subscribe` enumerates them and
/// frames flow. A tree-fetch failure is non-fatal: we fall back to the caller's `input`
/// unchanged (the pump still arms; it just carries nothing until the engine is reachable),
/// so a transient engine blip never fails the whole `watch`.
async fn expand_scope(base: &str, input: &Value) -> Value {
    let scope = input.get("scope");
    let has_components = scope
        .and_then(|s| s.get("components"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    let has_properties = scope
        .and_then(|s| s.get("properties"))
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty());
    if has_components || has_properties {
        return input.clone(); // Explicit scope — never widen it.
    }

    let uids = match raw_tree::run(base, input).await {
        Ok(tree) => scope_uids::collect(&tree),
        Err(_) => return input.clone(), // Engine unreachable → arm with the given scope.
    };
    if uids.is_empty() {
        return input.clone(); // Nothing to watch (blank appliance) → leave as-is.
    }

    // Merge the expanded components into a fresh `scope`, preserving any `tick_hz` the
    // caller set. We rebuild rather than mutate in place to keep `input` borrow-free.
    let mut expanded = input.clone();
    let obj = expanded
        .as_object_mut()
        .expect("watch input is a JSON object");
    let mut new_scope = scope.cloned().unwrap_or_else(|| json!({}));
    new_scope["components"] = json!(uids);
    obj.insert("scope".into(), new_scope);
    expanded
}
