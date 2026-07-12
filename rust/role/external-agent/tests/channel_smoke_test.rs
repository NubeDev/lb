//! Opt-in **end-to-end in-channel** run (channels-agent + external-agent): post a real `kind:"agent"`
//! item into a real channel on a real `Node`, selecting the **external** `open-interpreter-default`
//! runtime, drive the background drain, and assert the worker posts a real `agent_result` carrying the
//! agent's answer. This is the whole headline path — channel `post` (enqueues the durable run) →
//! background reactor drain → agent worker → runtime seam → Open Interpreter subprocess →
//! **Z.AI GLM-4.6** → `agent_result` in durable history — with NOTHING faked (rule 9; the model is the
//! real Z.AI coding endpoint). (`post` returns before the run; `drain_channel_agent_runs` is the
//! synchronous flush the background reactor performs on its tick — run-lifecycle #5.)
//!
//! Gated on `EXTAGENT_SMOKE=1` (like the seam smoke) so the default offline `cargo test` skips it: it
//! needs the `interpreter` binary on `PATH` and a non-throttled `ZAI_API_KEY`. With those set:
//!   EXTAGENT_SMOKE=1 ZAI_API_KEY=… cargo test -p lb-role-external-agent --test channel_smoke_test -- --nocapture

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, drain_channel_agent_runs, history, post, AgentConfig, ModelEndpointPatch,
    Node, RuntimeRegistry, UnconfiguredModel,
};
use lb_inbox::Item;
use lb_role_external_agent::profiles::{default_model_endpoint, OPEN_INTERPRETER_DEFAULT};
use lb_role_external_agent::register;
use std::sync::Arc;

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:ada".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn asking_in_a_channel_drives_open_interpreter_against_zai() {
    if std::env::var("EXTAGENT_SMOKE").ok().as_deref() != Some("1") {
        eprintln!("EXTAGENT_SMOKE!=1 — skipping in-channel external run (offline default)");
        return;
    }

    let node = Arc::new(Node::boot().await.expect("node boots"));

    // Install the external runtimes on the node exactly as the feature-on `node` binary does at boot.
    let mut registry = RuntimeRegistry::with_default(Arc::new(UnconfiguredModel));
    register(&mut registry, default_model_endpoint(), None);
    assert!(
        registry
            .ids()
            .iter()
            .any(|id| id == OPEN_INTERPRETER_DEFAULT),
        "the external default is registered"
    );
    node.install_runtimes(registry);

    let ws = "extagent-channel-smoke";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            "mcp:agent.invoke:call",
        ],
    );

    // The request: ask the EXTERNAL agent, in the channel, to reply PONG. `job` is the run id.
    let body = serde_json::json!({
        "kind": "agent",
        "goal": "Reply with exactly: PONG",
        "runtime": OPEN_INTERPRETER_DEFAULT,
        "job": "run-channel-smoke",
    })
    .to_string();
    post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await
    .expect("the agent request posts");

    // `post` only ENQUEUED the durable run (run-lifecycle #5) — drive the drain the background reactor
    // would run on its tick, synchronously, so the smoke test observes the completed answer.
    drain_channel_agent_runs(&node, ws).await;

    // The worker's answer is in durable history, correlated to the run.
    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let result = items
        .iter()
        .find(|i| i.author == "system:agent-worker")
        .expect("the agent worker posted a result/error");
    let parsed: serde_json::Value =
        serde_json::from_str(&result.body).expect("the worker body is JSON");
    assert_eq!(
        parsed["kind"], "agent_result",
        "the external run succeeded (not an agent_error): {parsed}"
    );
    assert_eq!(parsed["runtime"], OPEN_INTERPRETER_DEFAULT);
    let answer = parsed["answer"].as_str().unwrap_or_default();
    assert!(
        !answer.trim().is_empty(),
        "the agent produced a non-empty answer (fail loud on a throttled/mis-keyed run)"
    );
    eprintln!("in-channel external agent answered: {answer:?}");
}

/// The **sealed-key** end-to-end (agent-catalog test-and-secrets scope): the model key is set via the
/// real `lb-secrets` store (as the self-serve `secret.set` API does) and referenced from the
/// workspace's `agent.config.api_key_secret` — the key is then REMOVED from the process env, so the
/// ONLY place it exists is the sealed secret. The external run must still reach Z.AI GLM-4.6, proving
/// `AcpRuntime::run` resolves the sealed key (host-mediated) and `drive` injects it into the child env.
/// This closes the loop: "a user adds the token through the API → the real run uses it", with NO
/// dependency on the operator's node env.
///
/// Same gate (`EXTAGENT_SMOKE=1` + a live `ZAI_API_KEY` seed value + `interpreter` on PATH). Run it
/// alone (`--test-threads=1`) — it mutates the process env, and restores it before returning.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_sealed_key_from_the_config_reaches_the_external_run() {
    if std::env::var("EXTAGENT_SMOKE").ok().as_deref() != Some("1") {
        eprintln!("EXTAGENT_SMOKE!=1 — skipping sealed-key external run (offline default)");
        return;
    }
    // The real key to seal — read ONCE from the env the operator provided, then removed below so the
    // run cannot fall back to it. If it's unset, there's nothing to prove; skip loudly.
    let Ok(real_key) = std::env::var("ZAI_API_KEY") else {
        eprintln!("ZAI_API_KEY unset — cannot seed the sealed-key smoke; skipping");
        return;
    };

    let node = Arc::new(Node::boot().await.expect("node boots"));
    let mut registry = RuntimeRegistry::with_default(Arc::new(UnconfiguredModel));
    register(&mut registry, default_model_endpoint(), None);
    node.install_runtimes(registry);

    let ws = "extagent-sealed-key-smoke";
    let cid = "ops";
    let p = principal(
        ws,
        &[
            &format!("bus:chan/{cid}:pub"),
            &format!("bus:chan/{cid}:sub"),
            "mcp:agent.invoke:call",
            "mcp:agent.config.set:call",
            "secret:agent/*:write",
        ],
    );

    // (1) SEAL the key through the real secrets store. `Workspace` visibility so the run's DERIVED
    //     `agent:` principal (not this admin) can resolve it at model-call time (the UI seals it the
    //     same way). The value lands ONLY in lb-secrets.
    let secret_path = "agent/sealed-model-key";
    lb_secrets::set_with(
        &node.store,
        &p,
        ws,
        secret_path,
        &real_key,
        lb_secrets::Visibility::Workspace,
    )
    .await
    .expect("seal the model key");

    // (2) POINT the workspace's active config at that sealed path (names-only — a path, no value).
    agent_config_set(
        &node,
        &p,
        ws,
        &AgentConfig {
            active_definition: None,
            active_persona: None,
            enabled_personas: None,
            compact_budget: None,
            loop_window: None,
            exfiltration_guard: None,
            default_runtime: Some(OPEN_INTERPRETER_DEFAULT.to_string()),
            model_endpoint: Some(ModelEndpointPatch {
                api_key_secret: Some(secret_path.to_string()),
                ..Default::default()
            }),
        },
    )
    .await
    .expect("write agent.config with the sealed path");

    // (3) REMOVE the key from the process env — the sealed secret is now its ONLY home. If the run
    //     works, the value came from the seal (resolved by `AcpRuntime::run`, injected by `drive`).
    // SAFETY: this test is serialised (it mutates process env) and restores the var before returning.
    std::env::remove_var("ZAI_API_KEY");

    let body = serde_json::json!({
        "kind": "agent",
        "goal": "Reply with exactly: PONG",
        "runtime": OPEN_INTERPRETER_DEFAULT,
        "job": "run-sealed-key-smoke",
    })
    .to_string();
    let post_res = post(
        &node,
        &p,
        ws,
        cid,
        Item::new("q1", cid, "user:ada", body, 1),
    )
    .await;
    drain_channel_agent_runs(&node, ws).await;

    // Restore the env for any sibling test BEFORE asserting (so a panic can't leak the removal).
    std::env::set_var("ZAI_API_KEY", &real_key);
    post_res.expect("the agent request posts");

    let items = history(&node.store, &p, ws, cid).await.expect("history");
    let result = items
        .iter()
        .find(|i| i.author == "system:agent-worker")
        .expect("the agent worker posted a result/error");
    let parsed: serde_json::Value =
        serde_json::from_str(&result.body).expect("the worker body is JSON");
    assert_eq!(
        parsed["kind"], "agent_result",
        "the run keyed ONLY by the sealed secret succeeded (not an agent_error): {parsed}"
    );
    let answer = parsed["answer"].as_str().unwrap_or_default();
    assert!(
        !answer.trim().is_empty(),
        "GLM-4.6 answered using the SEALED key alone (no process env) — the loop is closed"
    );
    eprintln!("sealed-key external agent answered: {answer:?}");
}
