//! `proof-panel` — the Tier-1 WASM proof extension. It carries ONE real MCP tool, `proof.ping`,
//! served through the WASM component runtime (not a native sidecar). Where `fleet-monitor` proves the
//! native (Tier-2) path, this proves the WASM (Tier-1) path: publish-then-install → load through the
//! component runtime → a tool call routed through the host's capability gate reaches this guest.
//!
//! `proof.ping` is stateless: the reply is a pure function of the input (a well-behaved extension keeps
//! nothing in the instance, §3.4), so a hot-reload swap loses nothing. It returns a workspace-tagged
//! snapshot — the WASM analogue of `fleet.summary`.
//!
//! Workspace note (the honest Tier-1 difference from the native sidecar): a native child reads its
//! injected `LB_EXT_WS` env, but the **WIT `call(name, input-json)` ABI gives a wasm guest no ambient
//! identity** — only the JSON the host hands it. So the host caller supplies `ws` in the input (the same
//! way `series.latest` takes a `series` arg), and the guest echoes it back into the snapshot. The real
//! per-workspace wall is NOT this echoed field — it is the host's `mcp:proof-panel.proof.ping:call`
//! capability gate, re-checked against the *caller's token* before this guest is ever reached. The echo
//! proves reachability + round-trip, not authority. The UI half does NOT bind to this verb (the frozen
//! bridge contract is series-read-only); the tool exists to prove a WASM extension ships a real,
//! reachable backend tool alongside its federated page, in one folder.
//!
//! `proof.derive` (host-callback scope, `@0.2.0`) is the second tool — and the proof a guest does REAL
//! platform work. It uses the new `host.call-tool` import to (1) read the latest `proof.demo` sample via
//! `series.latest`, (2) write `proof.derived = value * 2` via `ingest.write`, returning `{"derived":N}`.
//! Both callbacks are authorized HOST-SIDE against the guest's `caller ∩ install-grant` effective
//! principal — the guest can only reach what both its caller and its install allow. The guest holds no
//! store/bus handle; it touches the platform ONLY through the mediated callback (rules 4/5).

// The `generate!` call is emitted by `build.rs` into `$OUT_DIR/wit_gen.rs`, reading the WIT from the
// standalone `lb-sdk` crate (the authoritative owner) — see the build script. Generated against the
// SAME WIT the host uses, so the ABI cannot drift.
include!(concat!(env!("OUT_DIR"), "/wit_gen.rs"));

use serde::{Deserialize, Serialize};

/// Input to `proof.ping` — the caller's workspace, echoed back into the snapshot. Optional: a caller
/// that omits it (e.g. a smoke probe) still gets a well-formed reply with an empty `ws`.
#[derive(Deserialize, Default)]
struct PingIn {
    #[serde(default)]
    ws: String,
}

/// Output of `proof.ping` — the workspace-tagged, runtime-tagged snapshot. Stateless: a pure function
/// of the input, so a hot-reload swap loses nothing (§3.4).
#[derive(Serialize)]
struct PingOut {
    ok: bool,
    ws: String,
    node: &'static str,
    tier: &'static str,
}

/// The series this demo reads from and writes to. Names are data, not identity (the workspace wall is
/// the host gate around every callback).
const SOURCE_SERIES: &str = "proof.demo";
const DERIVED_SERIES: &str = "proof.derived";

/// Output of `proof.derive` — the value it committed to `proof.derived`.
#[derive(Serialize)]
struct DeriveOut {
    derived: f64,
    source_seq: u64,
}

struct ProofPanel;

impl exports::lazybones::ext::tool::Guest for ProofPanel {
    fn call(
        name: String,
        input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        // Stateless (§3.4): no instance state; everything comes from the call.
        lazybones::ext::host::log(&format!("proof-panel.{name} called"));
        match name.as_str() {
            "proof.ping" => {
                // An empty input object is valid (ws defaults to ""); only malformed JSON is BadInput.
                let parsed: PingIn = serde_json::from_str(&input_json)
                    .map_err(|e| ToolError::BadInput(e.to_string()))?;
                let out = PingOut {
                    ok: true,
                    ws: parsed.ws,
                    node: "proof-panel",
                    tier: "wasm",
                };
                serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
            }
            // The host-callback proof: read a real series and write a derived one, ALL through the
            // host-mediated `host.call-tool` import (host-callback scope). No store handle, no token.
            "proof.derive" => derive(),
            // The workflow-simulation proof (proof-workflow-sim scope): the guest DRIVES a full
            // inbox→approval→outbox round-trip ENTIRELY through the host callback — it PRODUCES the
            // motion the page then sees, instead of only reading something else seeded.
            "proof.simulate" => simulate(),
            // A self-recursive tool: it calls ITSELF through the host callback. Exists ONLY to prove
            // the host's re-entrancy depth guard fires (the chain is refused with "call depth
            // exceeded" before any stack blow-up or lock deadlock). It never terminates on its own —
            // the guard is what stops it.
            "proof.recurse" => {
                use lazybones::ext::host::call_tool;
                match call_tool("proof-panel.proof.recurse", "{}") {
                    Ok(_) => Ok(r#"{"recursed":true}"#.to_string()),
                    // The host deny/limit surfaces here; bubble it as a failure so the guard is visible.
                    Err(lazybones::ext::host::ToolError::Failed(m)) => {
                        Err(ToolError::Failed(format!("host: {m}")))
                    }
                    Err(lazybones::ext::host::ToolError::BadInput(m)) => {
                        Err(ToolError::BadInput(format!("host: {m}")))
                    }
                }
            }
            // An unknown tool is an explicit error — never a silent success (mirrors fleet-monitor).
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

/// `proof.derive` — the guest doing real platform work through the host callback.
///
/// 1. `host.call-tool("series.latest", {"series":"proof.demo"})` → the newest source sample.
/// 2. derive `value * 2`.
/// 3. `host.call-tool("ingest.write", {"samples":[…proof.derived…]})` → commit it.
///
/// Each callback is authorized host-side against the guest's `caller ∩ install-grant` principal — if
/// the install grant omits `ingest.write` (or the caller lacks it), step 3 is DENIED at the host even
/// though the guest asked, and that denial surfaces here as a `Failed` (never silently swallowed).
fn derive() -> Result<String, exports::lazybones::ext::tool::ToolError> {
    use exports::lazybones::ext::tool::ToolError;
    use lazybones::ext::host::{call_tool, ToolError as HostErr};

    // map a host-callback error onto the guest's tool error so a denial is an honest failure.
    fn host_err(e: HostErr) -> ToolError {
        match e {
            HostErr::BadInput(m) => ToolError::BadInput(format!("host: {m}")),
            HostErr::Failed(m) => ToolError::Failed(format!("host: {m}")),
        }
    }

    // 1. Read the latest source sample through the callback.
    let latest_in = serde_json::json!({ "series": SOURCE_SERIES }).to_string();
    let latest_out = call_tool("series.latest", &latest_in).map_err(host_err)?;
    let latest: serde_json::Value =
        serde_json::from_str(&latest_out).map_err(|e| ToolError::Failed(e.to_string()))?;
    let sample = latest.get("sample");
    if sample.is_none() || sample == Some(&serde_json::Value::Null) {
        return Err(ToolError::Failed(format!(
            "no '{SOURCE_SERIES}' sample to derive from"
        )));
    }
    let sample = sample.unwrap();
    let value = sample
        .get("payload")
        .and_then(|p| p.as_f64())
        .ok_or_else(|| ToolError::Failed("source payload is not a number".into()))?;
    // Reuse the source seq so the derived row is an idempotent UPSERT (re-deriving the same source
    // point overwrites, never duplicates — the host commits on `[series, producer, seq]`).
    let source_seq = sample.get("seq").and_then(|s| s.as_u64()).unwrap_or(0);

    // 2 + 3. Write the derived sample through the callback (producer is overridden host-side).
    let derived = value * 2.0;
    let write_in = serde_json::json!({
        "samples": [{
            "series": DERIVED_SERIES,
            "producer": "proof-panel",
            "ts": 0,
            "seq": source_seq,
            "payload": derived,
        }]
    })
    .to_string();
    call_tool("ingest.write", &write_in).map_err(host_err)?;

    serde_json::to_string(&DeriveOut {
        derived,
        source_seq,
    })
    .map_err(|e| ToolError::Failed(e.to_string()))
}

/// The channel `proof.simulate` produces its triage item on. Dedicated to the simulation so its items
/// are self-contained; the page's InboxSection reads this same channel so the produced item is visible.
const TRIAGE_CHANNEL: &str = "proof-triage";
/// A stable item id — re-running the simulation upserts the SAME inbox item (idempotent, no duplicate
/// pile-up). The host commits on `(channel, id)`.
const SIM_ITEM_ID: &str = "proof-sim-item";
/// A stable outbox effect id — re-running upserts the same pending effect (idempotent on the id).
const SIM_EFFECT_ID: &str = "proof-sim-effect";
/// A logical ordering ts the guest injects (no wall-clock in a wasm guest — the host has no clock for
/// it either; the inbox/outbox ts is logical, testing §3). Fixed because the ids are idempotent.
const SIM_TS: u64 = 1;

/// Output of `proof.simulate` — a summary the page renders to show EACH step landed: the inbox item's
/// id, that it was resolved Approved, and the resulting outbox pending count.
#[derive(Serialize)]
struct SimulateOut {
    inbox_id: String,
    resolved: bool,
    outbox_pending: u64,
}

/// `proof.simulate` — the guest drives a full inbox→approval→outbox round-trip through the host callback
/// (proof-workflow-sim scope). It PRODUCES the workflow motion the page then sees:
///
/// 1. `inbox.record` an item on `proof-triage` (author host-forced to the effective principal's sub).
/// 2. `inbox.list` it back, find the item by id (proving the host committed it — not the guest's word).
/// 3. `inbox.resolve` that id Approved (the durable workflow WRITE).
/// 4. `outbox.enqueue` an effect keyed off the approval (a pending must-deliver intent).
/// 5. `outbox.status` to read the resulting pending count.
///
/// Each callback authorizes host-side against `caller ∩ install-grant`. If the install grant omits (or
/// the caller lacks) any verb, that step is DENIED at the host and surfaces here as a `Failed` — never
/// a silent skip or a fabricated summary.
fn simulate() -> Result<String, exports::lazybones::ext::tool::ToolError> {
    use exports::lazybones::ext::tool::ToolError;
    use lazybones::ext::host::{call_tool, ToolError as HostErr};

    fn host_err(e: HostErr) -> ToolError {
        match e {
            HostErr::BadInput(m) => ToolError::BadInput(format!("host: {m}")),
            HostErr::Failed(m) => ToolError::Failed(format!("host: {m}")),
        }
    }

    // 1. Record an inbox item (the host forces the author to the principal's sub; we pass a body + ts).
    let record_in = serde_json::json!({
        "channel": TRIAGE_CHANNEL,
        "id": SIM_ITEM_ID,
        "body": "proof.simulate: please approve this simulated request",
        "ts": SIM_TS,
    })
    .to_string();
    call_tool("inbox.record", &record_in).map_err(host_err)?;

    // 2. List the channel back and find our item (a SEPARATE host read confirms the write committed).
    let list_in = serde_json::json!({ "channel": TRIAGE_CHANNEL }).to_string();
    let list_out = call_tool("inbox.list", &list_in).map_err(host_err)?;
    let listed: serde_json::Value =
        serde_json::from_str(&list_out).map_err(|e| ToolError::Failed(e.to_string()))?;
    let found = listed
        .get("items")
        .and_then(|v| v.as_array())
        .map(|items| items.iter().any(|i| i.get("id").and_then(|x| x.as_str()) == Some(SIM_ITEM_ID)))
        .unwrap_or(false);
    if !found {
        return Err(ToolError::Failed(format!(
            "recorded item '{SIM_ITEM_ID}' did not appear in '{TRIAGE_CHANNEL}'"
        )));
    }

    // 3. Resolve it Approved — the durable workflow write.
    let resolve_in = serde_json::json!({
        "item_id": SIM_ITEM_ID,
        "decision": "approved",
        "ts": SIM_TS + 1,
    })
    .to_string();
    call_tool("inbox.resolve", &resolve_in).map_err(host_err)?;

    // 4. Enqueue an outbox effect keyed off the approval (a pending must-deliver intent).
    let enqueue_in = serde_json::json!({
        "id": SIM_EFFECT_ID,
        "target": "demo",
        "action": "comment",
        "payload": format!("approved {SIM_ITEM_ID} via proof.simulate"),
        "ts": SIM_TS + 2,
    })
    .to_string();
    call_tool("outbox.enqueue", &enqueue_in).map_err(host_err)?;

    // 5. Read the resulting pending count (a SEPARATE host read).
    let status_out = call_tool("outbox.status", "{}").map_err(host_err)?;
    let status: serde_json::Value =
        serde_json::from_str(&status_out).map_err(|e| ToolError::Failed(e.to_string()))?;
    let outbox_pending = status
        .get("pending")
        .and_then(|v| v.as_array())
        .map(|p| p.len() as u64)
        .unwrap_or(0);

    serde_json::to_string(&SimulateOut {
        inbox_id: SIM_ITEM_ID.to_string(),
        resolved: true,
        outbox_pending,
    })
    .map_err(|e| ToolError::Failed(e.to_string()))
}

export!(ProofPanel);

// Unit tests for the pure dispatch body. These exercise the SAME `match` the WIT export drives, on the
// host target (no wasm runtime needed) — the ok / unknown-tool-is-error / bad-params-is-error shape the
// proof-panel scope requires, mirroring `fleet-monitor/src/call.rs`. The end-to-end "real component
// through lb-runtime" proof lives in `crates/host/tests/proof_panel_test.rs`.
#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatch under test, decoupled from the generated WIT `Guest::call` (which is only callable
    /// from a wasm host). Identical logic; kept in one place so the test and the export cannot drift.
    fn dispatch(name: &str, input_json: &str) -> Result<String, String> {
        match name {
            "proof.ping" => {
                let parsed: PingIn =
                    serde_json::from_str(input_json).map_err(|e| format!("bad-input: {e}"))?;
                let out = PingOut {
                    ok: true,
                    ws: parsed.ws,
                    node: "proof-panel",
                    tier: "wasm",
                };
                serde_json::to_string(&out).map_err(|e| e.to_string())
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    #[test]
    fn ping_returns_a_workspace_tagged_wasm_snapshot() {
        let out = dispatch("proof.ping", r#"{"ws":"acme"}"#).expect("ping ok");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["ws"], "acme", "the caller's workspace round-trips");
        assert_eq!(v["node"], "proof-panel");
        assert_eq!(v["tier"], "wasm", "served by the Tier-1 component");
    }

    #[test]
    fn ping_with_empty_input_defaults_the_workspace() {
        // `{}` is valid input — a smoke probe still gets a well-formed reply (ws defaults to "").
        let out = dispatch("proof.ping", "{}").expect("ping ok on empty object");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["ws"], "");
        assert_eq!(v["tier"], "wasm");
    }

    #[test]
    fn unknown_tool_is_an_explicit_error() {
        let err = dispatch("proof.delete", "{}").expect_err("unknown tool must error");
        assert!(err.contains("unknown tool"), "got {err}");
    }

    #[test]
    fn bad_params_is_an_error_not_a_panic() {
        let err = dispatch("proof.ping", "not json").expect_err("malformed input must error");
        assert!(err.contains("bad-input"), "got {err}");
    }
}
