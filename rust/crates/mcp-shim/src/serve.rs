//! The JSON-RPC 2.0 stdio loop. Reads one request per line from stdin, dispatches by method,
//! writes one response per line to stdout. Three methods are handled:
//!
//! - `initialize` → MCP server capabilities + info. No auth needed (the menu is untrusted
//!   advertisement; the wall fires per-call on the gateway).
//! - `tools/list` → the pre-baked menu (read once at startup, returned verbatim). Advertisement
//!   only — `caps::check` on every `tools/call` is the wall.
//! - `tools/call` → forward to the gateway under the run-scoped token, refreshing if due / on a
//!   401 (D2), and return the MCP content-block result.
//!
//! Non-JSON lines and unknown methods produce a JSON-RPC error response (`-32601`), never a
//! crash — an agent that emits a stray banner line must not break the run. A request without an
//! `id` is a notification (no response) per the spec.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;

use crate::config::{idle_timeout, read_env, EnvConfig};
use crate::forward::{call_gateway, ForwardError};
use crate::menu::{load_menu, MenuEntry};
use crate::refresh::Refresher;

/// The MCP protocol version this shim speaks. Pinned to `2024-11-05` (the spec codex/Open
/// Interpreter target). A mismatch surfaces as a client-side warning, never a hard failure — the
/// three methods here are stable across the recent spec revisions.
const PROTOCOL_VERSION: &str = "2024-11-05";

/// Read env, build the refresher + menu, and run the stdio loop until stdin closes or the idle
/// timeout fires. The whole shim is one call to this — the bin entry is just the runtime.
pub async fn serve() -> Result<(), String> {
    let cfg = read_env()?;
    let menu = load_menu(std::path::Path::new(&cfg.menu_path))?;
    let client = reqwest::Client::builder()
        .build()
        .map_err(|e| format!("build http client: {e}"))?;
    let refresher = Refresher::new(
        cfg.gateway_url.clone(),
        cfg.run_id.clone(),
        cfg.token.clone(),
        cfg.refresh_at,
        client.clone(),
    );
    serve_with(
        cfg,
        menu,
        refresher,
        client,
        tokio::io::stdin(),
        tokio::io::stdout(),
    )
    .await
}

/// The loop over a reader/writer pair — public so an integration test can drive a real gateway
/// through the shim's actual dispatch path without spawning the binary. The test boots a real
/// gateway, creates a `tokio::io::duplex`, calls `serve_on`, writes JSON-RPC requests to the
/// duplex's write half, and reads responses from the read half. No fake backend — the HTTP call
/// to `/mcp/call` hits the real gateway (rule 9).
pub async fn serve_on<R, W>(
    menu: Vec<MenuEntry>,
    refresher: Refresher,
    client: reqwest::Client,
    gateway_url: String,
    stdin: R,
    stdout: W,
) -> Result<(), String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let cfg = EnvConfig {
        gateway_url,
        token: refresher.token().await,
        run_id: String::new(),
        menu_path: String::new(),
        refresh_at: None,
    };
    serve_with(cfg, menu, refresher, client, stdin, stdout).await
}

/// The loop over a reader/writer pair — split out so a test can drive a pair of pipes without
/// touching the process's real stdio.
async fn serve_with<R, W>(
    _cfg: EnvConfig,
    menu: Vec<MenuEntry>,
    refresher: Refresher,
    client: reqwest::Client,
    stdin: R,
    mut stdout: W,
) -> Result<(), String>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut lines = BufReader::new(stdin).lines();
    loop {
        let next = match timeout(idle_timeout(), lines.next_line()).await {
            Ok(Ok(Some(line))) => line,
            Ok(Ok(None)) => return Ok(()), // stdin closed — agent exited
            Ok(Err(e)) => return Err(format!("read stdin: {e}")),
            Err(_) => return Err("idle timeout".into()),
        };
        let gateway_url = refresher.gateway_url();
        let response = handle_line(&next, &menu, &refresher, &client, &gateway_url).await;
        let response = match response {
            Some(out) => out,
            None => continue, // a notification (no `id`) — no response per spec
        };
        let mut out = serde_json::to_string(&response).map_err(|e| e.to_string())?;
        out.push('\n');
        stdout
            .write_all(out.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        stdout.flush().await.map_err(|e| e.to_string())?;
    }
}

#[derive(Debug, Deserialize)]
struct Request {
    #[serde(rename = "jsonrpc")]
    _jsonrpc: Option<String>,
    /// The id — a number or a string. Absent ⇒ notification (no response).
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

#[derive(Debug, Serialize)]
struct RpcError {
    code: i32,
    message: String,
}

impl RpcError {
    const METHOD_NOT_FOUND: i32 = -32601;
    const INVALID_PARAMS: i32 = -32602;
    const INTERNAL: i32 = -32603;
}

/// Dispatch one inbound line. `None` ⇒ a notification, no response. `Some(json-value)` ⇒ the
/// JSON-RPC response object to write back.
fn handle_line<'a>(
    line: &'a str,
    menu: &'a [MenuEntry],
    refresher: &'a Refresher,
    client: &'a reqwest::Client,
    gateway_url: &'a str,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<Value>> + Send + 'a>> {
    Box::pin(async move {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(_) => {
                // Not even valid JSON — no id, no response (the spec allows dropping garbage).
                return None;
            }
        };
        let Some(id) = req.id.clone() else {
            // A notification — handle for side effects if we ever need them; today we ignore.
            return None;
        };
        let result = match req.method.as_str() {
            "initialize" => Ok(json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "lb-mcp-shim", "version": "0.1.0" },
            })),
            "tools/list" => Ok(json!({ "tools": menu })),
            "tools/call" => handle_call(&req.params, refresher, client, gateway_url).await,
            _ => Err(RpcError {
                code: RpcError::METHOD_NOT_FOUND,
                message: format!("unknown method: {}", req.method),
            }),
        };
        Some(serde_response(id, result))
    })
}

/// Serialize a `Result<Value, RpcError>` into the JSON-RPC response object.
fn serde_response(id: Value, result: Result<Value, RpcError>) -> Value {
    match result {
        Ok(v) => json!({ "jsonrpc": "2.0", "id": id, "result": v }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    }
}

/// Handle one `tools/call`: parse `{name, arguments}`, refresh if due, forward, refresh+retry
/// once on 401, and turn the gateway's reply into the MCP content-block result.
async fn handle_call(
    params: &Value,
    refresher: &Refresher,
    client: &reqwest::Client,
    gateway_url: &str,
) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError {
            code: RpcError::INVALID_PARAMS,
            message: "missing `name`".into(),
        })?;
    let args = params.get("arguments").cloned().unwrap_or(Value::Null);

    refresher.begin_call().await;
    // Proactive refresh (D2) before the first attempt.
    let token = match refresher.token_refreshed_if_due().await {
        Ok(t) => t,
        Err(ForwardError::Unauthorized) => {
            // The proactive refresh itself was refused (run terminal) → fail closed.
            return Ok(tool_error("run token no longer valid (run ended)"));
        }
        Err(e) => {
            return Err(RpcError {
                code: RpcError::INTERNAL,
                message: e.to_string(),
            })
        }
    };
    match call_gateway(client, gateway_url, &token, name, &args).await {
        Ok(outcome) => Ok(serde_json::to_value(&outcome).unwrap_or(Value::Null)),
        Err(ForwardError::Unauthorized) => {
            // One-shot self-heal (D2): refresh + retry exactly once.
            let healed = match refresher.heal_once().await {
                Ok(t) => t,
                Err(ForwardError::Unauthorized) => {
                    return Ok(tool_error("run token expired and refresh was refused"));
                }
                Err(e) => {
                    return Err(RpcError {
                        code: RpcError::INTERNAL,
                        message: e.to_string(),
                    })
                }
            };
            match call_gateway(client, gateway_url, &healed, name, &args).await {
                Ok(outcome) => Ok(serde_json::to_value(&outcome).unwrap_or(Value::Null)),
                // Second 401 in the same call → fail closed (the race window is one retry wide).
                Err(ForwardError::Unauthorized) => Ok(tool_error(
                    "run token expired (refresh did not clear the 401)",
                )),
                Err(e) => Err(RpcError {
                    code: RpcError::INTERNAL,
                    message: e.to_string(),
                }),
            }
        }
        Err(e) => Err(RpcError {
            code: RpcError::INTERNAL,
            message: e.to_string(),
        }),
    }
}

/// An MCP `isError` result carrying a plain text reason (used for soft fail-closed outcomes —
/// an expired token or a refused refresh). The agent renders it as a tool error, not a crash.
fn tool_error(text: &str) -> Value {
    json!({
        "content": [{ "type": "text", "text": text }],
        "isError": true,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MenuEntry;

    #[tokio::test]
    async fn initialize_returns_capabilities() {
        let menu = vec![MenuEntry::name_only("tools.catalog")];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let line = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#;
        let out = handle_line(line, &menu, &r, &reqwest::Client::new(), &gw).await;
        let resp = out.expect("initialize has a response");
        assert_eq!(resp["id"], 1);
        assert_eq!(resp["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(resp["result"]["serverInfo"]["name"], "lb-mcp-shim");
    }

    #[tokio::test]
    async fn tools_list_returns_the_menu() {
        let menu = vec![
            MenuEntry::name_only("tools.catalog"),
            MenuEntry::name_only("devkit.scaffold"),
        ];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let line = r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#;
        let out = handle_line(line, &menu, &r, &reqwest::Client::new(), &gw).await;
        let resp = out.expect("tools/list has a response");
        let tools = resp["result"]["tools"].as_array().expect("tools is array");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0]["name"], "tools.catalog");
    }

    #[tokio::test]
    async fn unknown_method_is_method_not_found() {
        let menu = vec![];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let line = r#"{"jsonrpc":"2.0","id":"x","method":"files/read","params":{}}"#;
        let out = handle_line(line, &menu, &r, &reqwest::Client::new(), &gw).await;
        let resp = out.expect("has a response");
        assert_eq!(resp["error"]["code"], RpcError::METHOD_NOT_FOUND);
        assert_eq!(resp["id"], "x");
    }

    #[tokio::test]
    async fn garbage_line_is_dropped_silently() {
        let menu = vec![];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let out = handle_line("not json at all", &menu, &r, &reqwest::Client::new(), &gw).await;
        assert!(out.is_none(), "a non-JSON line produces no response");
    }

    #[tokio::test]
    async fn notification_without_id_gets_no_response() {
        let menu = vec![];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let line = r#"{"jsonrpc":"2.0","method":"notifications/initialized","params":{}}"#;
        let out = handle_line(line, &menu, &r, &reqwest::Client::new(), &gw).await;
        assert!(out.is_none(), "a notification (no id) gets no response");
    }

    #[tokio::test]
    async fn tools_call_missing_name_is_invalid_params() {
        let menu = vec![];
        let client = reqwest::Client::new();
        let r = Refresher::new(
            "http://127.0.0.1:1".into(),
            "run".into(),
            "t".into(),
            None,
            client,
        );
        let gw = r.gateway_url();
        let line = r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"arguments":{}}}"#;
        let out = handle_line(line, &menu, &r, &reqwest::Client::new(), &gw).await;
        let resp = out.expect("has a response");
        assert_eq!(resp["error"]["code"], RpcError::INVALID_PARAMS);
    }
}
