//! Opt-in **real-subprocess** run through the `AcpRuntime` seam — gated on `EXTAGENT_SMOKE=1` so the
//! default `cargo test` stays offline (rule 9: the real path is exercised, but not in CI without an
//! agent binary + a non-throttled provider key). Mirrors the leaf crate's `vtcode_smoke_test` posture.
//!
//! With the env set, it boots a real Node, builds an `AcpRuntime` for the default external agent
//! (Open Interpreter → `interpreter`, or override via `EXTAGENT_PROFILE`), and drives it against the
//! goal, forwarding `RunEvent`s. It asserts the run reached a terminal event — failing loud on an
//! empty stream (same strictness the leaf smoke keeps), so it can never pass on nothing.
//!
//! Z.AI GLM-4.6 facts (verified): reach it via `ZAI_API_KEY` + the coding endpoint (the provider
//! wiring lives in the codex `model_providers` `-c` overrides the wrapper/profile carry; see the
//! session doc). This test does not re-probe — it just runs what the profile configures.

use lb_host::{AllowedTool, Node, RunContext, RuntimeRegistry};
use lb_role_external_agent::profiles::{default_model_endpoint, OPEN_INTERPRETER_DEFAULT};
use lb_role_external_agent::register;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn drives_a_real_external_agent_through_the_seam() {
    if std::env::var("EXTAGENT_SMOKE").ok().as_deref() != Some("1") {
        eprintln!("EXTAGENT_SMOKE!=1 — skipping real-subprocess run (offline default)");
        return;
    }
    let profile_id =
        std::env::var("EXTAGENT_PROFILE").unwrap_or_else(|_| OPEN_INTERPRETER_DEFAULT.to_string());

    let node = std::sync::Arc::new(Node::boot().await.expect("node boots"));
    let ws = "extagent-smoke";
    let caller = lb_auth::Principal::routed("user:smoke", ws, vec!["mcp:agent.invoke:call".into()]);

    // Build a registry the way a feature-ON node does, then resolve the external runtime.
    let mut registry = RuntimeRegistry::with_default(null_model());
    register(&mut registry, default_model_endpoint(), None);
    let runtime = registry
        .resolve(Some(&profile_id))
        .expect("external profile registered");

    let ctx = RunContext {
        ws,
        job_id: "smoke-1",
        goal: "Reply with exactly: PONG",
        caller: &caller,
        agent_caps: &["mcp:agent.invoke:call".to_string()],
        tools: &[] as &[AllowedTool],
        model_override: None,
        persona_catalog: None,
        persona_preset: None,
        ts: 1,
    };
    let answer = runtime
        .run(&node, ctx)
        .await
        .expect("the real run completes");
    // Fail loud on an empty answer — a throttled/mis-keyed run must not pass on nothing.
    assert!(!answer.trim().is_empty(), "the agent produced some text");
    eprintln!("external agent ({profile_id}) answered: {answer:?}");
}

fn null_model() -> std::sync::Arc<dyn lb_host::ErasedModel> {
    struct M;
    impl lb_host::ErasedModel for M {
        fn turn_boxed<'a>(
            &'a self,
            _ws: &'a str,
            _m: &'a [(String, String)],
            _t: &'a [AllowedTool],
            _p: &'a [lb_host::CallOutcome],
            _k: &'a str,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<lb_host::Turn, lb_host::TurnError>> + Send + 'a>>
        {
            Box::pin(async {
                Ok(lb_host::Turn {
                    content: String::new(),
                    calls: vec![],
                    done: true,
                })
            })
        }

        fn is_configured(&self) -> bool {
            false
        }
    }
    std::sync::Arc::new(M)
}
