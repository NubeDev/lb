//! `record_dispatch` — emit the **redacted** dispatch event through `tracing` (observability scope).
//! The host tool-dispatch chokepoint (`lb_host::call_tool`, README §6.5) calls this immediately
//! after the cap decision, so every mediated action — host service or WASM guest — is observed
//! **without cooperating** (the guest cannot opt out of being observed, the same property that makes
//! it capability-checked here). This is the single highest-leverage span in the system.
//!
//! Redaction is structural here, not advisory: `params` arrive ONLY as `params_digest` (SHA-256 +
//! shape, never the raw value), and the `Secret<T>` type can never reach a field. So the planted-
//! value redaction test (telemetry-console scope) passes by construction — a leak would require the
//! caller to pass the raw secret as a field, which the type forbids.

use crate::record::{Level, Outcome};
use crate::redact::params_digest;

/// The `tracing` target every telemetry event uses, so the `SurrealCappedLayer` (and only it) picks
/// these events up without coupling to every other crate's instrumentation.
pub const TARGET: &str = "lb.telemetry";

/// Emit the dispatch event with the full schema. `params` is the raw tool params — it is digested
/// HERE, never stored. `source` is the emitting crate/extension (e.g. `host`, `mqtt`). `outcome` is
/// the cap decision (`Allow`/`Deny`) or `Error` for a tool that ran but failed. `ts` is the
/// logical timestamp the caller threads (no wall-clock in core).
#[allow(clippy::too_many_arguments)]
pub fn record_dispatch(
    level: Level,
    ws: &str,
    actor: &str,
    tool: &str,
    source: &str,
    trace_id: &str,
    outcome: Outcome,
    params: &serde_json::Value,
    ts: u64,
    msg: &str,
) {
    // The semantic level rides as a field; the tracing event itself is emitted at INFO so the
    // subscriber's LevelFilter sees it (the stored `level` field is what the console filters on).
    let digest = params_digest(params);
    tracing::event!(
        target: TARGET,
        tracing::Level::INFO,
        lvl = %level.as_str(),
        ws = %ws,
        actor = %actor,
        tool = %tool,
        source = %source,
        trace_id = %trace_id,
        outcome = %outcome.as_str(),
        params_digest = %digest,
        ts = ts,
        msg = %msg,
    );
}
