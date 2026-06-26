//! `hello` v2 — the SAME `echo` tool as v1, but its output carries `"v": 2`. This is the
//! swap target for the hot-reload test (testing §2.4): loading this over v1 live must change
//! the answer (proving the instance was replaced) while NO durable state is lost — because a
//! well-behaved extension keeps nothing in the instance (stateless-extension, §3.4). All
//! state lives in the store/bus, so a swap is safe.

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
    v: u8,
}

struct Hello;

impl exports::lazybones::ext::tool::Guest for Hello {
    fn call(
        name: String,
        input_json: String,
    ) -> Result<String, exports::lazybones::ext::tool::ToolError> {
        use exports::lazybones::ext::tool::ToolError;
        lazybones::ext::host::log(&format!("hello-v2.{name} called"));
        match name.as_str() {
            "echo" => {
                let parsed: EchoIn = serde_json::from_str(&input_json)
                    .map_err(|e| ToolError::BadInput(e.to_string()))?;
                let out = EchoOut {
                    echo: parsed.msg,
                    v: 2,
                };
                serde_json::to_string(&out).map_err(|e| ToolError::Failed(e.to_string()))
            }
            other => Err(ToolError::Failed(format!("unknown tool: {other}"))),
        }
    }
}

export!(Hello);
