//! The `control-engine.*` verb dispatch (folder-of-verbs, one file per verb).
//!
//! `dispatch` maps a manifest tool NAME (the cap gate — house rule) + its parsed
//! input to one `ControlEngine` trait call and returns the verbatim serde JSON
//! result. It is the seam the crate's own unit tests drive against `ce_fake` (with
//! its call counter) to prove dispatch + verbatim-DTO behaviour without a process.
//!
//! Deny is enforced HOST-side on the tool name (via `authorize_tool`) BEFORE the
//! sidecar is ever called, so a denied call reaches neither `dispatch` nor the CE —
//! the crate unit test asserts that dispatch invokes the trait exactly once per
//! allowed call (counter semantics) while the host integration test asserts the
//! `Denied` at the `call_tool` boundary.

pub mod schema;
pub mod tree;

use rubix_ce::{ControlEngine, EngineInstanceId};
use serde_json::Value;

/// Dispatch one `control-engine.*` verb against a bound CE client.
///
/// `tool` is the full manifest tool name; `input` is the already-parsed argument
/// object. Returns the verb's verbatim JSON result, or an error string (mapped by
/// `main` onto a `Reply::err`). Unknown tools error — the host only ever routes the
/// declared names, so this is a defensive fallback.
pub async fn dispatch(
    engine: &dyn ControlEngine,
    instance: &EngineInstanceId,
    tool: &str,
    input: &Value,
) -> Result<Value, String> {
    match tool {
        "control-engine.tree" => tree::run(engine, instance, input).await,
        "control-engine.schema" => schema::run(engine).await,
        other => Err(format!("unknown tool: {other}")),
    }
}
