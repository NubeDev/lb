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
use lb_host::{drain_channel_agent_runs, history, post, Node, RuntimeRegistry, UnconfiguredModel};
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
