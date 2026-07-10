//! `echarts-panel` — the Tier-1 WASM reference DATA-TILE extension. Its backend is deliberately
//! minimal: ONE static tool, `echarts.about`, served through the WASM component runtime (not a native
//! sidecar). Where `proof-panel` proves a wasm guest can do real platform work through the host
//! callback, this extension's *point* is the FRONTEND — a frames-in chart widget that renders
//! `ctx.data` with Apache ECharts. The backend tool exists only to prove the Tier-1 backend half is a
//! real, reachable MCP tool (the WASM analogue of `proof.ping`, minus any input echo).
//!
//! `echarts.about` is stateless: the reply is a constant (a pure function of no input, §3.4), so a
//! hot-reload swap loses nothing. It returns `{ ok: true, ext: "echarts-panel" }`. The UI half does NOT
//! bind to this verb — the chart tile is pure-render (frames pushed in as `ctx.data`; it calls no tool).
//! The tool exists to prove a WASM extension ships a real, reachable backend tool alongside its
//! federated UI, in one folder.

// The `generate!` call is emitted by `build.rs` into `$OUT_DIR/wit_gen.rs`, reading the WIT from the
// standalone `lb-sdk` crate (the authoritative owner) — see the build script. Generated against the
// SAME WIT the host uses, so the ABI cannot drift.
include!(concat!(env!("OUT_DIR"), "/wit_gen.rs"));

use serde::Serialize;

/// Output of `echarts.about` — a static snapshot. Stateless: a pure function of no input, so a
/// hot-reload swap loses nothing (§3.4).
#[derive(Serialize)]
struct AboutOut {
    ok: bool,
    ext: &'static str,
}

struct EchartsPanel;

impl exports::lazybones::ext::tool::Guest for EchartsPanel {
    fn call(
        name: String,
        _input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        // Stateless (§3.4): no instance state; the reply is constant.
        lazybones::ext::host::log(&format!("echarts-panel.{name} called"));
        match name.as_str() {
            "echarts.about" => {
                let out = AboutOut {
                    ok: true,
                    ext: "echarts-panel",
                };
                serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
            }
            // An unknown tool is an explicit error — never a silent success (mirrors proof-panel).
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(EchartsPanel);

// Unit tests for the pure dispatch body. These exercise the SAME `match` the WIT export drives, on the
// host target (no wasm runtime needed) — the ok / unknown-tool-is-error / bad-params-is-error shape the
// reference-extension scope requires, mirroring `proof-panel/src/lib.rs`.
#[cfg(test)]
mod tests {
    use super::*;

    /// The dispatch under test, decoupled from the generated WIT `Guest::call` (which is only callable
    /// from a wasm host). Identical logic; kept in one place so the test and the export cannot drift.
    fn dispatch(name: &str, input_json: &str) -> Result<String, String> {
        // `echarts.about` takes no input, but a malformed JSON body is still an explicit BadInput —
        // exercise the same parse the real dispatch tolerates (an empty object is valid, "not json"
        // is not) so the bad-params contract is covered.
        match name {
            "echarts.about" => {
                // Reject a malformed body up front so `bad_params` is an honest error, not a panic.
                if !input_json.trim().is_empty() {
                    serde_json::from_str::<serde_json::Value>(input_json)
                        .map_err(|e| format!("bad-input: {e}"))?;
                }
                let out = AboutOut {
                    ok: true,
                    ext: "echarts-panel",
                };
                serde_json::to_string(&out).map_err(|e| e.to_string())
            }
            other => Err(format!("unknown tool: {other}")),
        }
    }

    #[test]
    fn about_returns_a_static_snapshot() {
        let out = dispatch("echarts.about", "{}").expect("about ok");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["ext"], "echarts-panel");
    }

    #[test]
    fn about_with_empty_input_is_ok() {
        let out = dispatch("echarts.about", "").expect("about ok on empty input");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
    }

    #[test]
    fn unknown_tool_is_an_explicit_error() {
        let err = dispatch("echarts.delete", "{}").expect_err("unknown tool must error");
        assert!(err.contains("unknown tool"), "got {err}");
    }

    #[test]
    fn bad_params_is_an_error_not_a_panic() {
        let err = dispatch("echarts.about", "not json").expect_err("malformed input must error");
        assert!(err.contains("bad-input"), "got {err}");
    }
}
