//! **Live** end-to-end test against the REAL Z.AI `zaicoding` endpoint (active-agent-wiring scope).
//! This is the one thing the in-process HTTP test (`openai_compat_test.rs`) cannot prove: that the
//! shipped `AiGateway<OpenAiCompat>` — built exactly as `node/src/agent.rs::adapter_for` builds it for
//! provider `zaicoding` — actually authenticates and completes a turn against Z.AI's live server.
//!
//! **Network-gated, not a fake (CLAUDE §9).** It hits the real provider (the one true external we may
//! stub elsewhere), so it runs ONLY when `ZAI_LIVE_KEY` is set — CI without the key skips it, no fake
//! stands in. Run it with:
//!
//! ```sh
//! ZAI_LIVE_KEY=<token> cargo test -p lb-role-ai-gateway --test zai_live_test -- --ignored --nocapture
//! ```

use lb_host::{AllowedTool, ModelAccess};
use lb_role_ai_gateway::{AiGateway, OpenAiCompat};
use serde_json::json;

/// The exact base URL the `zaicoding` built-in binds (`agents.toml`), and the model id.
const ZAI_BASE_URL: &str = "https://api.z.ai/api/coding/paas/v4";
const ZAI_MODEL: &str = "glm-4.6";

#[tokio::test]
#[ignore = "hits the live Z.AI endpoint; set ZAI_LIVE_KEY to run"]
async fn a_live_zai_glm46_turn_completes_with_content() {
    let Ok(key) = std::env::var("ZAI_LIVE_KEY") else {
        eprintln!("ZAI_LIVE_KEY unset — skipping the live Z.AI turn");
        return;
    };

    // Build the SAME model the node builds for provider `zaicoding` (adapter_for → OpenAiCompat).
    let model = AiGateway::new(OpenAiCompat::new(
        key,
        ZAI_MODEL.to_string(),
        Some(ZAI_BASE_URL.to_string()),
    ));

    // One real self-describe turn — no tools, no prior calls.
    let turn = model
        .turn(
            "ws-live",
            &[("user".into(), "Reply with exactly the word: PONG".into())],
            &[],
            &[],
            "live-1",
        )
        .await;

    eprintln!("live Z.AI answer: {:?}", turn.content);
    eprintln!("done={} calls={}", turn.done, turn.calls.len());

    // The turn must complete with real content — not an empty string, and not the honest
    // "model call failed: …" the adapter returns on an API/auth error.
    assert!(
        !turn.content.trim().is_empty(),
        "the live turn returned empty content"
    );
    assert!(
        !turn.content.starts_with("model call failed"),
        "the live turn hit an API/auth error: {}",
        turn.content
    );
    // The stripped answer must not still carry a raw think tag.
    assert!(
        !turn.content.contains("<think>") && !turn.content.contains("</think>"),
        "a <think> tag leaked into the live answer: {}",
        turn.content
    );
}

#[tokio::test]
#[ignore = "hits the live Z.AI endpoint; set ZAI_LIVE_KEY to run"]
async fn a_live_turn_with_a_real_tool_schema_proposes_a_call_not_prose() {
    // The behavioral regression: given a tool WITH a real input schema, a capable model should
    // PROPOSE the call (fill the args) rather than ask the user in prose. This is the live proof that
    // threading `input_schema` through to `function.parameters` fixes "the agent just asks questions".
    let Ok(key) = std::env::var("ZAI_LIVE_KEY") else {
        eprintln!("ZAI_LIVE_KEY unset — skipping");
        return;
    };
    let model = AiGateway::new(OpenAiCompat::new(
        key,
        ZAI_MODEL.to_string(),
        Some(ZAI_BASE_URL.to_string()),
    ));

    let tools = vec![AllowedTool {
        name: "datasource_list".into(),
        description:
            "List the workspace's registered datasources. Call this to discover datasources.".into(),
        input_schema: Some(json!({ "type": "object", "properties": {}, "required": [] })),
    }];

    let turn = model
        .turn(
            "ws-live",
            &[(
                "user".into(),
                "What datasources do I have? Use the tools available to find out.".into(),
            )],
            &tools,
            &[],
            "live-tools-1",
        )
        .await;

    eprintln!(
        "live tool turn: content={:?} calls={:?}",
        turn.content,
        turn.calls.iter().map(|c| &c.name).collect::<Vec<_>>()
    );
    assert!(
        !turn.calls.is_empty(),
        "with a real tool schema the model should PROPOSE datasource_list, not answer in prose: {:?}",
        turn.content
    );
    assert_eq!(turn.calls[0].name, "datasource_list");
}
