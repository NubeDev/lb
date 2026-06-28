//! agent-run scope Part 4 — a **real spawned ACP adapter** speaking real JSON-RPC over a real stdio
//! pipe (the `pnpm test:gateway` pattern — a real process, no fake; rule 9). The ONLY stub is the
//! model provider, fed deterministically to the spawned binary via `LB_ACP_MOCK_SCRIPT` (testing §3).
//!
//! We drive the full lifecycle: `initialize` → `session/new` → `session/prompt` → streamed
//! `session/update`s → a terminal `stopReason`. Plus the clean rejection of client-provided
//! `mcpServers` (Resolved decision: rejected, not silently dropped).

use std::process::Stdio;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;

/// Spawn the real `lb-acp` binary with a deterministic provider script + workspace, returning its
/// child handle. The script: one stop turn that answers "hello from acp".
fn spawn_adapter(script: &str, tools: &str) -> tokio::process::Child {
    Command::new(env!("CARGO_BIN_EXE_lb-acp"))
        .env("LB_ACP_WS", "acp-ws")
        .env("LB_ACP_USER", "user:dev")
        .env("LB_ACP_TOOLS", tools)
        .env("LB_ACP_MOCK_SCRIPT", script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn lb-acp")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_real_adapter_drives_new_prompt_and_streams_updates_to_a_stop() {
    // The model just answers (no tool calls) — one stop turn.
    let script =
        r#"[{"content":"hello from acp","tool_calls":[],"finish_reason":"stop","tokens":3}]"#;
    let mut child = spawn_adapter(script, "");
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    // 1. initialize
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
    )
    .await;
    let init = read_until_id(&mut lines, 1).await;
    assert_eq!(init["result"]["protocolVersion"], 1, "handshake: {init}");

    // 2. session/new
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"sessionId":"run1"}}"#,
    )
    .await;
    let new = read_until_id(&mut lines, 2).await;
    assert_eq!(new["result"]["sessionId"], "run1");

    // 3. session/prompt → streamed session/update(s) then a response with a stopReason.
    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"run1","prompt":"hi"}}"#,
    )
    .await;

    // Collect frames until the id=3 response; assert at least one session/update carried the text.
    let mut saw_text = false;
    loop {
        let frame = next_frame(&mut lines).await.expect("a frame");
        if frame.get("method").and_then(|m| m.as_str()) == Some("session/update") {
            let update = &frame["params"]["update"];
            if update["sessionUpdate"] == "agent_message_chunk"
                && update["content"]["text"].as_str() == Some("hello from acp")
            {
                saw_text = true;
            }
        } else if frame.get("id").and_then(|i| i.as_u64()) == Some(3) {
            assert_eq!(
                frame["result"]["stopReason"], "end_turn",
                "prompt response: {frame}"
            );
            break;
        }
    }
    assert!(
        saw_text,
        "the adapter streamed the assistant text as a session/update"
    );

    drop(stdin);
    let _ = child.wait().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn client_provided_mcp_servers_are_rejected_cleanly() {
    // Resolved decision: a client `mcpServers` on session/new is REJECTED with a clean ACP error
    // (not silently dropped) — bridging client-side tools needs a future net:* grant.
    let script = r#"[{"content":"x","tool_calls":[],"finish_reason":"stop","tokens":1}]"#;
    let mut child = spawn_adapter(script, "");
    let mut stdin = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();

    send(
        &mut stdin,
        r#"{"jsonrpc":"2.0","id":1,"method":"session/new","params":{"sessionId":"run1","mcpServers":[{"name":"x"}]}}"#,
    )
    .await;
    let resp = read_until_id(&mut lines, 1).await;
    assert!(
        resp.get("error").is_some(),
        "client mcpServers must be rejected with an error, got: {resp}"
    );
    assert_eq!(
        resp["error"]["code"], -32010,
        "the unsupported-client-servers code"
    );

    drop(stdin);
    let _ = child.wait().await;
}

// --- tiny JSON-RPC-over-stdio helpers (real bytes, real process) ---

async fn send(stdin: &mut tokio::process::ChildStdin, frame: &str) {
    stdin.write_all(frame.as_bytes()).await.unwrap();
    stdin.write_all(b"\n").await.unwrap();
    stdin.flush().await.unwrap();
}

async fn next_frame(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
) -> Option<serde_json::Value> {
    let line = lines.next_line().await.ok()??;
    serde_json::from_str(&line).ok()
}

/// Read frames until one with the given response id; ignores interleaved notifications.
async fn read_until_id(
    lines: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    id: u64,
) -> serde_json::Value {
    for _ in 0..64 {
        if let Some(frame) = next_frame(lines).await {
            if frame.get("id").and_then(|i| i.as_u64()) == Some(id) {
                return frame;
            }
        } else {
            break;
        }
    }
    panic!("no response with id {id}");
}
