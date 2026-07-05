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

use lb_host::ModelAccess;
use lb_role_ai_gateway::{AiGateway, OpenAiCompat};

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
            &[(
                "user".into(),
                "Reply with exactly the word: PONG".into(),
            )],
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
}
