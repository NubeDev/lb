//! The real OpenAI-compatible adapter, end to end over REAL HTTP against a REAL in-process
//! chat-completions server (active-agent-wiring scope, Slice 1). The provider HTTP is the one true
//! external, so here it is a real `axum` server on a real TCP port — not a `*.fake.ts`/in-code
//! re-implementation of the model (CLAUDE §9 / testing §0). We drive the whole stack the node uses:
//! `AiGateway<OpenAiCompat>` (the gateway + idempotency cache is what `complete` really is).
//!
//! Cases:
//!   - **happy turn:** a canned completion → content + tokens flow through as a `stop`.
//!   - **tool-call turn:** `finish_reason:"tool_calls"` → one carried `ToolCall`, `ToolCalls`.
//!   - **API error:** a 500 → a terminal `stop` attributed `"model call failed: …/{model}: …"`
//!     (honest failure — not a panic, not an empty answer).
//!   - **request shape:** the server actually received `Authorization: Bearer <key>`, the right
//!     `model`, and the messages (captured and asserted).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use lb_role_ai_gateway::{AiGateway, AiRequest, FinishReason, Message, OpenAiCompat};
use serde_json::{json, Value};

/// What the scripted server saw and what it should answer. Shared so a test asserts on the request
/// the real adapter sent.
#[derive(Clone)]
struct Server {
    /// The last request body + auth header the handler received.
    seen: Arc<Mutex<Option<Seen>>>,
    /// The status the handler replies with (500 exercises the error path).
    status: u16,
    /// The JSON body the handler replies with on success.
    reply: Value,
}

struct Seen {
    body: Value,
    authorization: Option<String>,
}

async fn chat_completions(
    State(server): State<Server>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> impl IntoResponse {
    *server.seen.lock().unwrap() = Some(Seen {
        body,
        authorization: headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string),
    });
    let status = axum::http::StatusCode::from_u16(server.status).unwrap();
    (status, Json(server.reply.clone()))
}

/// Boot the scripted chat-completions server on a real port; return `(base_url, seen-handle)`.
async fn serve(status: u16, reply: Value) -> (String, Arc<Mutex<Option<Seen>>>) {
    let seen = Arc::new(Mutex::new(None));
    let server = Server {
        seen: seen.clone(),
        status,
        reply,
    };
    let app = Router::new()
        .route("/chat/completions", post(chat_completions))
        .with_state(server);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), seen)
}

/// A one-user-message request through the gateway to `base_url` with `key`/`model`.
fn request() -> AiRequest {
    let mut req = AiRequest::new("ws-oai", "k1");
    req.messages = vec![Message::new("user", "say hi")];
    req
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn happy_turn_flows_content_and_tokens_through() {
    let (base, _seen) = serve(
        200,
        json!({
            "choices": [{
                "message": { "content": "hello from glm" },
                "finish_reason": "stop"
            }],
            "usage": { "total_tokens": 42 }
        }),
    )
    .await;

    let gw = AiGateway::new(OpenAiCompat::new(
        "sk-test".into(),
        "glm-4".into(),
        Some(base),
    ));
    let resp = gw.complete(&request()).await;

    assert_eq!(resp.content, "hello from glm");
    assert_eq!(resp.finish_reason, FinishReason::Stop);
    assert_eq!(resp.tokens, 42);
    assert!(resp.tool_calls.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tool_call_turn_carries_the_proposed_call() {
    let (base, _seen) = serve(
        200,
        json!({
            "choices": [{
                "message": {
                    "content": "",
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "hello.echo", "arguments": "{\"msg\":\"hi\"}" }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": { "total_tokens": 7 }
        }),
    )
    .await;

    let gw = AiGateway::new(OpenAiCompat::new(
        "sk-test".into(),
        "glm-4".into(),
        Some(base),
    ));
    let resp = gw.complete(&request()).await;

    assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].id, "call_1");
    assert_eq!(resp.tool_calls[0].name, "hello.echo");
    assert_eq!(resp.tool_calls[0].input, "{\"msg\":\"hi\"}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_error_is_an_attributed_terminal_stop() {
    // A 500 must not panic and must not look like a real empty completion — it is a terminal stop
    // whose content names the fault and the model (honest-failure contract).
    let (base, _seen) = serve(500, json!({ "error": "boom" })).await;

    let gw = AiGateway::new(OpenAiCompat::new(
        "sk-test".into(),
        "glm-4".into(),
        Some(base),
    ));
    let resp = gw.complete(&request()).await;

    assert_eq!(resp.finish_reason, FinishReason::Stop);
    assert_eq!(resp.tokens, 0);
    assert!(
        resp.content.contains("model call failed"),
        "attributed failure, got: {}",
        resp.content
    );
    assert!(
        resp.content.contains("glm-4"),
        "names the model, got: {}",
        resp.content
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_server_receives_the_right_auth_model_and_messages() {
    let (base, seen) = serve(
        200,
        json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }],
            "usage": { "total_tokens": 1 }
        }),
    )
    .await;

    let gw = AiGateway::new(OpenAiCompat::new(
        "sk-secret".into(),
        "glm-4".into(),
        Some(base),
    ));
    let _ = gw.complete(&request()).await;

    let seen = seen.lock().unwrap();
    let seen = seen.as_ref().expect("server received a request");
    assert_eq!(
        seen.authorization.as_deref(),
        Some("Bearer sk-secret"),
        "the adapter sent the bearer key"
    );
    assert_eq!(seen.body["model"], "glm-4");
    assert_eq!(seen.body["messages"][0]["role"], "user");
    assert_eq!(seen.body["messages"][0]["content"], "say hi");
    // No tools in this request → the field is omitted entirely.
    assert!(
        seen.body.get("tools").is_none(),
        "tools omitted when empty, got: {}",
        seen.body
    );
}
