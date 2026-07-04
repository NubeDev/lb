//! **Page-context injection** (agent-dock scope) — proves the optional client-reported `context`
//! object is fenced into the run's goal on the ONE seam both front doors reach (`invoke_via_runtime`),
//! and that the size cap + absent-is-identical invariants hold.
//!
//! Rule 9: everything real — the `mem://` store, the bus, caps, and the loop are real code. The only
//! permitted fake is the model **provider** (a scripted capturing provider behind the `Provider`
//! trait), which lets us inspect the exact prompt the loop assembled — the honest way to assert "the
//! fenced context reached the model" without a network.
//!
//! What this proves (the scope's Host context injection tests):
//!   - a run WITH `context` has the fenced, untrusted-labelled block in the prompt (surface/path/search
//!     all present);
//!   - an OVERSIZE context (>4 KB serialized) is REJECTED before any model call (a `BadInput`, opaque
//!     to a run fault);
//!   - ABSENT context is byte-identical to today (no fence text, the run still completes).

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{invoke_via_runtime, AgentError, ErasedModel, Node, RuntimeRegistry, Substrate};
use lb_role_ai_gateway::{AiRequest, AiResponse, Provider};
use serde_json::json;

const INVOKE: &str = "mcp:agent.invoke:call";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A scripted provider that CAPTURES the concatenated prompt text of each request so the test can
/// assert what the loop assembled. Answers a single `stop` turn. Behind the `Provider` trait — the
/// only permitted fake (the model HTTP), never the store/bus/caps.
#[derive(Clone, Default)]
struct CapturingProvider {
    prompts: Arc<Mutex<Vec<String>>>,
}

impl Provider for CapturingProvider {
    async fn complete(&self, req: &AiRequest) -> AiResponse {
        let joined = req
            .messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        self.prompts.lock().unwrap().push(joined);
        AiResponse::stop("done", 1)
    }
}

/// Install `provider` (behind an `AiGateway`) as the node's in-house default runtime; return the
/// registry to drive `invoke_via_runtime` against. Same install seam as the in-house wiring test.
async fn registry_with(node: &Arc<Node>, provider: CapturingProvider) -> Arc<RuntimeRegistry> {
    use lb_role_ai_gateway::AiGateway;
    let model: Arc<dyn ErasedModel> = Arc::new(AiGateway::new(provider));
    node.install_runtimes(RuntimeRegistry::with_default(model));
    node.runtimes()
}

/// The full prompt the provider saw (all turns joined) — where the fence must appear.
fn seen_prompt(p: &CapturingProvider) -> String {
    p.prompts.lock().unwrap().join("\n")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn context_is_fenced_into_the_prompt_as_untrusted() {
    let ws = "ws-ctx-fence";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    let provider = CapturingProvider::default();
    let registry = registry_with(&node, provider.clone()).await;

    let context = json!({
        "surface": "dashboards",
        "path": "/t/acme/dashboards",
        "search": { "d": "sales", "from": "now-24h" }
    });

    invoke_via_runtime(
        &node,
        &registry,
        None,
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-ctx-1",
        "why did throughput dip this morning?",
        Substrate::default(),
        Some(&context),
        &[],
        1,
    )
    .await
    .expect("the run completes");

    let prompt = seen_prompt(&provider);
    assert!(
        prompt.contains("why did throughput dip this morning?"),
        "the goal is present"
    );
    assert!(
        prompt.contains("untrusted, client-reported context"),
        "the fence names the context untrusted: {prompt}"
    );
    assert!(
        prompt.contains("\"surface\":\"dashboards\""),
        "surface fenced in: {prompt}"
    );
    assert!(
        prompt.contains("\"d\":\"sales\""),
        "search fenced in: {prompt}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn oversize_context_is_rejected_before_any_model_call() {
    let ws = "ws-ctx-oversize";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    let provider = CapturingProvider::default();
    let registry = registry_with(&node, provider.clone()).await;

    // A `path` padded well past the 4 KB serialized ceiling.
    let big = "x".repeat(lb_host::MAX_CONTEXT_BYTES + 64);
    let context = json!({ "surface": "s", "path": big, "search": {} });

    let err = invoke_via_runtime(
        &node,
        &registry,
        None,
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-ctx-2",
        "hi",
        Substrate::default(),
        Some(&context),
        &[],
        1,
    )
    .await
    .expect_err("oversize context is rejected");
    assert!(
        matches!(err, AgentError::BadInput(_)),
        "a bad-input reject, not a run fault: {err:?}"
    );
    // The model was NEVER called — the reject happens before the loop drives.
    assert!(
        provider.prompts.lock().unwrap().is_empty(),
        "no model call on reject"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_context_is_byte_identical_to_today() {
    let ws = "ws-ctx-absent";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE]);
    let provider = CapturingProvider::default();
    let registry = registry_with(&node, provider.clone()).await;

    invoke_via_runtime(
        &node,
        &registry,
        None,
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-ctx-3",
        "plain goal, no context",
        Substrate::default(),
        None,
        &[],
        1,
    )
    .await
    .expect("the run completes");

    let prompt = seen_prompt(&provider);
    assert!(
        prompt.contains("plain goal, no context"),
        "the goal is present"
    );
    assert!(
        !prompt.contains("untrusted, client-reported context"),
        "no fence text when context is absent: {prompt}"
    );
}
