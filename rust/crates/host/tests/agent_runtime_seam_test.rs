//! External-agent sub-scope #1 (runtime-seam) — the HOST-side gate. Proves the `AgentRuntime` seam +
//! `RuntimeRegistry` + `invoke_via_runtime` selection against a **real** Node + the deterministic
//! MockProvider (rule 9 — the only stubbed external is the provider HTTP; store + bus + loop are
//! real). No external agent binary here (that is the role crate's opt-in smoke); this file locks the
//! seam's contract:
//!   - registry resolution (absent → default; known → entry; unknown named → error);
//!   - the DEFAULT runtime drives the SAME in-house loop through the seam (no second path) — the
//!     "default-unaffected" gate;
//!   - MANDATORY capability-deny (§2.1): `agent.invoke` denied without `mcp:agent.invoke:call`,
//!     identically through the seam as through `invoke`.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    invoke_via_runtime, AgentError, AllowedTool, ErasedModel, Node, RuntimeRegistry, Substrate,
    DEFAULT_RUNTIME,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const INVOKE: &str = "mcp:agent.invoke:call";

/// A model that stops immediately with a fixed answer (no tool calls) — enough to prove the loop ran
/// through the seam.
fn answer_model(answer: &str) -> Arc<dyn ErasedModel> {
    Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
        answer, 1,
    )])))
}

fn default_registry(answer: &str) -> RuntimeRegistry {
    RuntimeRegistry::with_default(answer_model(answer))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn absent_runtime_resolves_to_the_in_house_default() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "seam-default";
    let caller = principal("user:ada", ws, &[INVOKE]);
    let registry = default_registry("in-house answer");

    // No `runtime` arg → the default in-house loop runs through the seam and returns its answer.
    let answer = invoke_via_runtime(
        &node,
        &registry,
        None,
        None,
        &caller,
        &[],
        ws,
        "job-default",
        "do a thing",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect("default runtime runs");
    assert_eq!(answer, "in-house answer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn explicitly_named_default_resolves_the_same() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "seam-named-default";
    let caller = principal("user:ada", ws, &[INVOKE]);
    let registry = default_registry("in-house answer");

    let answer = invoke_via_runtime(
        &node,
        &registry,
        Some(DEFAULT_RUNTIME),
        None,
        &caller,
        &[],
        ws,
        "job-named-default",
        "do a thing",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect("named default runs");
    assert_eq!(answer, "in-house answer");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_explicitly_named_unknown_runtime_is_an_error_not_a_silent_downgrade() {
    // The decided resolution rule: a caller that asked for a specific runtime must NOT be silently
    // downgraded to the default engine. Feature-off / unconfigured node has no such entry → error.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "seam-unknown";
    let caller = principal("user:ada", ws, &[INVOKE]);
    let registry = default_registry("in-house answer");

    let err = invoke_via_runtime(
        &node,
        &registry,
        Some("open-interpreter-default"), // named, but this OFF-registry has only `default`
        None,
        &caller,
        &[],
        ws,
        "job-unknown",
        "do a thing",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("an unknown named runtime errors");
    assert!(
        matches!(err, AgentError::BadInput(m) if m.contains("open-interpreter-default")),
        "the error names the unknown runtime"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn invoke_is_denied_without_the_cap_identically_through_the_seam() {
    // MANDATORY capability-deny (§2.1): the invoke gate is the SAME for every runtime — choosing a
    // runtime is an argument, not a grant. A caller lacking `mcp:agent.invoke:call` is refused before
    // any runtime is selected (so an unknown runtime + missing cap is still a Denied, not BadInput).
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "seam-deny";
    let caller = principal("user:ada", ws, &[]); // no invoke cap
    let registry = default_registry("unreachable");

    let err = invoke_via_runtime(
        &node,
        &registry,
        None,
        None,
        &caller,
        &[],
        ws,
        "job-deny",
        "do a thing",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("ungranted invoke is denied");
    assert!(
        matches!(err, AgentError::Denied),
        "denied at the invoke gate, before runtime selection"
    );
}

#[test]
fn a_default_registry_lists_only_the_default() {
    // Feature-off posture: the registry a node without the external-agent feature builds holds ONLY
    // the in-house default. `agent.runtimes` (#5, TODO) reads this same list.
    let registry = default_registry("x");
    assert_eq!(registry.ids(), vec![DEFAULT_RUNTIME.to_string()]);
}
