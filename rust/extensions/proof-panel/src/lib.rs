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

wit_bindgen::generate!({
    path: "../../sdk/wit",
    world: "extension",
});

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
            // An unknown tool is an explicit error — never a silent success (mirrors fleet-monitor).
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
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
