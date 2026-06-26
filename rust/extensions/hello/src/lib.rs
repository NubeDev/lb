//! `hello` — the trivial S1 extension. One tool, `echo`, proving the whole spine end to end:
//! caller → MCP → caps → WIT → WASM → back (mcp scope). Generated against the SAME WIT the
//! host uses, so the ABI cannot drift.

wit_bindgen::generate!({
    path: "../../sdk/wit",
    world: "extension",
});

use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct EchoIn {
    msg: String,
}

#[derive(Serialize)]
struct EchoOut {
    echo: String,
}

struct Hello;

impl exports::lazybones::ext::tool::Guest for Hello {
    fn call(name: String, input_json: String) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        // Stateless (§3.4): no instance state; everything comes from the call.
        lazybones::ext::host::log(&format!("hello.{name} called"));
        match name.as_str() {
            "echo" => {
                let parsed: EchoIn = serde_json::from_str(&input_json)
                    .map_err(|e| ToolError::BadInput(e.to_string()))?;
                let out = EchoOut { echo: parsed.msg };
                serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
            }
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(Hello);
