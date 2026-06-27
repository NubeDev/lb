//! `proof-panel` — the Tier-1 WASM proof extension. It carries ONE real MCP tool, `proof.status`,
//! served through the WASM component runtime (not a native sidecar). Where `fleet-monitor` proves the
//! native (Tier-2) path, this proves the WASM (Tier-1) path: publish-then-install → load through the
//! component runtime → a tool call routed through the host's capability gate reaches this guest.
//!
//! `proof.status` is stateless: the reply is a pure function of the input (a well-behaved extension
//! keeps nothing in the instance, §3.4), so a hot-reload swap loses nothing. It echoes back the caller-
//! supplied `note` plus a fixed `tier: "wasm"` tag, proving the call reached *this* component and not
//! some other instance. The UI half does NOT bind to this verb (the frozen bridge contract is series-
//! read-only); the tool exists to prove a WASM extension ships a real, reachable backend tool.

wit_bindgen::generate!({
    path: "../../sdk/wit",
    world: "extension",
});

use serde::{Deserialize, Serialize};

/// Input to `proof.status` — a free-form note the caller wants echoed back, proving round-trip.
#[derive(Deserialize)]
struct StatusIn {
    note: String,
}

/// Output of `proof.status` — the echoed note plus the tier tag identifying the serving runtime.
#[derive(Serialize)]
struct StatusOut {
    ok: bool,
    note: String,
    tier: &'static str,
    ext: &'static str,
}

struct ProofPanel;

impl exports::lazybones::ext::tool::Guest for ProofPanel {
    fn call(
        name: String,
        input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        lazybones::ext::host::log(&format!("proof-panel.{name} called"));
        match name.as_str() {
            "proof.status" => {
                let parsed: StatusIn = serde_json::from_str(&input_json)
                    .map_err(|e| ToolError::BadInput(e.to_string()))?;
                let out = StatusOut {
                    ok: true,
                    note: parsed.note,
                    tier: "wasm",
                    ext: "proof-panel",
                };
                serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
            }
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(ProofPanel);
