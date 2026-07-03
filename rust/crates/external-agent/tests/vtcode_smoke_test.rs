//! Opt-in **real-subprocess** smoke test: spawns a genuine agent (vtcode by default, codex if asked)
//! and asserts the driver yields a well-formed `RunEvent` stream. Rule 9: no mock — a real binary, a
//! real pipe, driven through the real wrapper.
//!
//! Gated behind `VTCODE_SMOKE=1` (and a working provider key) because CI binary-availability +
//! provider quota is an open question the umbrella scope flags (#2/#3). Unset ⇒ no-op, so the default
//! `cargo test` stays green and offline — the posture the scope wants for an un-wired add-on.
//!
//! Run it locally:
//!   ZAI_API_KEY=... VTCODE_TRUST_WORKSPACE=full-auto VTCODE_SMOKE=1 \
//!     cargo test -p lb-external-agent --test vtcode_smoke_test -- --nocapture
//! Swap the agent with VTCODE_AGENT=codex (proving the driver is agent-agnostic).

use std::time::Duration;

use lb_external_agent::wrapper::AgentWrapper;
use lb_external_agent::{drive, AgentProfile, CodexWrapper, ModelEndpoint, VtcodeWrapper};
use lb_run_events::RunEvent;

#[tokio::test]
async fn agent_drives_a_real_run() {
    if std::env::var("VTCODE_SMOKE").as_deref() != Ok("1") {
        eprintln!(
            "skipping: set VTCODE_SMOKE=1 (and a provider key) to run the real-subprocess test"
        );
        return;
    }

    let model = ModelEndpoint {
        provider: std::env::var("VTCODE_PROVIDER").unwrap_or_else(|_| "zai".into()),
        model: std::env::var("VTCODE_MODEL").unwrap_or_else(|_| "glm-5.2".into()),
        api_key_env: std::env::var("VTCODE_KEY_ENV").unwrap_or_else(|_| "ZAI_API_KEY".into()),
        // vtcode takes provider/base_url via its own `--api-key-env` path, not codex `-c` overrides,
        // so its ModelEndpoint carries no base_url here (the codex wrapper is the only base_url reader).
        base_url: std::env::var("VTCODE_BASE_URL").ok(),
    };

    // Pick the wrapper + matching profile by env — the same call site drives either agent.
    let agent = std::env::var("VTCODE_AGENT").unwrap_or_else(|_| "vtcode".into());
    let (wrapper, profile): (&dyn AgentWrapper, AgentProfile) = match agent.as_str() {
        "codex" => (&CodexWrapper, AgentProfile::codex_default(model)),
        _ => (&VtcodeWrapper, AgentProfile::vtcode_default(model)),
    };

    let workspace = std::env::temp_dir().join("lb-agent-smoke");
    std::fs::create_dir_all(&workspace).unwrap();
    let workspace = workspace.to_str().unwrap();

    let events = drive(
        wrapper,
        &profile,
        "Print exactly the word PONG and nothing else. Do not use any tools.",
        workspace,
        Duration::from_secs(120),
        None, // no injected key — the standalone smoke reads the key from the process env (fallback)
        None, // no live sink in the standalone driver smoke — just collect + assert
    )
    .await
    .expect("agent run drove to completion");

    assert!(matches!(events.first(), Some(RunEvent::RunStart { .. })));
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RunEvent::TextDelta { .. } | RunEvent::RunFinish { .. })),
        "expected at least one text or finish event, got: {events:#?}"
    );
}
