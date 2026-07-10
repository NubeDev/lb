//! Generated Tier-1 WASM extension. Stateless by construction: every response is derived from the
//! call input and any platform access must go through host-mediated tools.

// The `generate!` call is emitted by `build.rs` into `$OUT_DIR/wit_gen.rs`, reading the WIT from the
// standalone `lb-sdk` crate (the authoritative owner) — see the build script. Generated against the
// SAME WIT the host uses, so the ABI cannot drift.
include!(concat!(env!("OUT_DIR"), "/wit_gen.rs"));

use serde::Serialize;

#[derive(Serialize)]
struct PingOut {
    ok: bool,
    ext: &'static str,
    tier: &'static str,
}

struct Extension;

impl exports::lazybones::ext::tool::Guest for Extension {
    fn call(
        name: String,
        _input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        lazybones::ext::host::log(&format!("energy-dashboard.{name} called"));
        match name.as_str() {
            "ping" => serde_json::to_string(&PingOut {
                ok: true,
                ext: "energy-dashboard",
                tier: "wasm",
            })
            .map_err(|e| ToolError::Failed(e.to_string())),
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(Extension);
