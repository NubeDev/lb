//! The gateway is stateless model access + a replay-safe idempotency cache (ai-gateway scope).
//! These prove the two S5 guarantees without a real provider (testing §3 — the mock is the only
//! external stubbed): the gateway returns the provider's turn, and a repeated idempotency key is
//! served from cache (NOT re-spent) — the property a resumed agent job depends on.

use lb_role_ai_gateway::{AiGateway, AiRequest, AiResponse, FinishReason, MockProvider, ToolCall};

#[tokio::test]
async fn returns_the_providers_turn() {
    let gw = AiGateway::new(MockProvider::new(vec![AiResponse::calls(
        "thinking",
        vec![ToolCall {
            id: "c1".into(),
            name: "hello.echo".into(),
            input: r#"{"msg":"hi"}"#.into(),
        }],
        10,
    )]));

    let req = AiRequest::new("ws-gw", "k1");
    let resp = gw.complete(&req).await;
    assert_eq!(resp.finish_reason, FinishReason::ToolCalls);
    assert_eq!(resp.tool_calls.len(), 1);
    assert_eq!(resp.tool_calls[0].name, "hello.echo");
}

#[tokio::test]
async fn a_repeated_idempotency_key_is_served_from_cache_not_re_spent() {
    // Two scripted turns. If the cache works, calling twice with the SAME key returns turn 0 both
    // times and the provider is invoked exactly once — a resumed job cannot re-spend or diverge.
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::stop("first", 5),
        AiResponse::stop("second", 5),
    ]));

    let req = AiRequest::new("ws-gw", "same-key");
    let a = gw.complete(&req).await;
    let b = gw.complete(&req).await; // resume: same key

    assert_eq!(a.content, "first");
    assert_eq!(b.content, "first", "cache replayed the first response");
    assert_eq!(
        gw.provider_calls(),
        1,
        "the provider was called once — the resume hit the cache"
    );
}

#[tokio::test]
async fn distinct_keys_advance_the_provider() {
    let gw = AiGateway::new(MockProvider::new(vec![
        AiResponse::stop("first", 5),
        AiResponse::stop("second", 5),
    ]));

    let first = gw.complete(&AiRequest::new("ws-gw", "k1")).await;
    let second = gw.complete(&AiRequest::new("ws-gw", "k2")).await;
    assert_eq!(first.content, "first");
    assert_eq!(second.content, "second");
    assert_eq!(gw.provider_calls(), 2);
}
