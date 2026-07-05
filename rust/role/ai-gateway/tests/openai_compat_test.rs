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
use lb_role_ai_gateway::{AiGateway, AiRequest, FinishReason, Message, OpenAiCompat, ToolSchema};
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tools_real_input_schema_reaches_the_provider_parameters() {
    // The regression the "works very bad" transcript surfaced: a tool was advertised with an EMPTY
    // parameter schema, so the model couldn't form a valid call and asked the user in prose. The
    // adapter must forward the tool's REAL input schema as the OpenAI function `parameters`.
    let (base, seen) = serve(
        200,
        json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }],
            "usage": { "total_tokens": 1 }
        }),
    )
    .await;

    let schema = json!({
        "type": "object",
        "properties": { "datasource": { "type": "string" } },
        "required": ["datasource"]
    });
    let mut req = request();
    req.tools = vec![ToolSchema {
        name: "datasource.list".into(),
        description: "list datasources".into(),
        parameters: Some(schema.clone()),
    }];

    let gw = AiGateway::new(OpenAiCompat::new("k".into(), "glm-4".into(), Some(base)));
    let _ = gw.complete(&req).await;

    let seen = seen.lock().unwrap();
    let seen = seen.as_ref().expect("server received a request");
    let fn_params = &seen.body["tools"][0]["function"]["parameters"];
    assert_eq!(
        fn_params, &schema,
        "the tool's real input schema must reach `function.parameters`, not an empty object"
    );
    assert_eq!(seen.body["tools"][0]["function"]["name"], "datasource.list");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_tool_with_no_schema_degrades_to_an_empty_object() {
    let (base, seen) = serve(
        200,
        json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }],
            "usage": { "total_tokens": 1 }
        }),
    )
    .await;

    let mut req = request();
    req.tools = vec![ToolSchema {
        name: "ping".into(),
        description: "no args".into(),
        parameters: None,
    }];

    let gw = AiGateway::new(OpenAiCompat::new("k".into(), "glm-4".into(), Some(base)));
    let _ = gw.complete(&req).await;

    let seen = seen.lock().unwrap();
    let seen = seen.as_ref().expect("server received a request");
    assert_eq!(
        seen.body["tools"][0]["function"]["parameters"],
        json!({ "type": "object" }),
        "a schemaless tool degrades to the OpenAI-required empty object"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_think_block_is_stripped_from_the_answer() {
    // GLM inlines `<think>…</think>` in content; it must not reach the answer.
    let (base, _seen) = serve(
        200,
        json!({
            "choices": [{
                "message": { "content": "<think>let me plan the query</think>Here is your chart." },
                "finish_reason": "stop"
            }],
            "usage": { "total_tokens": 7 }
        }),
    )
    .await;

    let gw = AiGateway::new(OpenAiCompat::new("k".into(), "glm-4".into(), Some(base)));
    let resp = gw.complete(&request()).await;

    assert_eq!(
        resp.content, "Here is your chart.",
        "the <think> reasoning block must be stripped from the model's answer"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn prior_results_are_folded_in_the_conformant_tool_call_shape() {
    // The wire shape the model was trained on: an assistant message echoing the originally-proposed
    // `tool_calls`, then a `role:"tool"` result keyed to each id. An orphan tool message (no echo)
    // is half-ignored — live GLM retried the identical rejected call three turns in a row. History
    // `role:"tool"` summaries must arrive as plain user text (`[tool results]…`), never orphans.
    let (base, seen) = serve(
        200,
        json!({
            "choices": [{ "message": { "content": "ok" }, "finish_reason": "stop" }],
            "usage": { "total_tokens": 1 }
        }),
    )
    .await;

    let gw = AiGateway::new(OpenAiCompat::new("k".into(), "glm-4".into(), Some(base)));
    let mut req = AiRequest::new("ws-oai", "k-prior");
    req.messages = vec![
        Message::new("user", "add a widget"),
        Message::new("assistant", "checking the datasource"),
        Message::new("tool", "call_0=ok:{\"datasources\":[]}"),
    ];
    req.prior_results = vec![lb_role_ai_gateway::ToolResult {
        id: "call_1".into(),
        name: "federation.query".into(),
        input: "{\"source\":\"timescale\",\"sql\":\"SELECT 1\"}".into(),
        ok: None,
        error: Some("bad input: rejected sql".into()),
    }];
    let _ = gw.complete(&req).await;

    let seen = seen.lock().unwrap();
    let messages = seen.as_ref().expect("request captured").body["messages"]
        .as_array()
        .expect("messages array")
        .clone();

    // The history tool summary was re-rolled as user text.
    assert_eq!(messages[2]["role"], "user");
    assert_eq!(
        messages[2]["content"],
        "[tool results]\ncall_0=ok:{\"datasources\":[]}"
    );

    // The prior result rides behind an assistant echo of the original call, keyed by id.
    let echo = &messages[3];
    assert_eq!(echo["role"], "assistant");
    assert_eq!(echo["tool_calls"][0]["id"], "call_1");
    assert_eq!(echo["tool_calls"][0]["function"]["name"], "federation.query");
    assert_eq!(
        echo["tool_calls"][0]["function"]["arguments"],
        "{\"source\":\"timescale\",\"sql\":\"SELECT 1\"}"
    );
    let result = &messages[4];
    assert_eq!(result["role"], "tool");
    assert_eq!(result["tool_call_id"], "call_1");
    assert_eq!(result["content"], "bad input: rejected sql");
}
