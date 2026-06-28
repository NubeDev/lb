//! Generated Tier-1 WASM extension. Stateless by construction: every response is derived from the
//! call input and any platform access must go through host-mediated tools.

wit_bindgen::generate!({
    path: "../../sdk/wit",
    world: "extension",
});

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
        lazybones::ext::host::log(&format!("devkit-build-wasm-2886399.{name} called"));
        match name.as_str() {
            "ping" => serde_json::to_string(&PingOut {
                ok: true,
                ext: "devkit-build-wasm-2886399",
                tier: "wasm",
            })
            .map_err(|e| ToolError::Failed(e.to_string())),
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(Extension);
