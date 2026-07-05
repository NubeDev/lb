//! Agent-personas sub-scope #4 (persona-coding) — the `builtin.extension-builder` persona's SAFETY
//! POSTURE, against the real Node + store + caps + loop (rule 9). "100% coding, but never on its own":
//! the persona builds EXTENSIONS via the devkit, supervised. This file proves the three posture
//! guarantees #4 adds on top of #1's record + application:
//!
//!   1. **Admin-tier / caps-deny (§2.1):** a MEMBER caller under the persona has `devkit.scaffold` /
//!      `ext.publish` DENIED at the wall — nothing scaffolds, nothing publishes (the persona narrows
//!      nothing into existence; the highest-stakes surface).
//!   2. **The Ask floor (`policy_preset`):** activating the persona makes `ext.publish` /
//!      `native.install` etc. evaluate to `Ask` (a durable human gate), while the edit/build inner
//!      loop stays `Allow`. **Floor semantics:** a blanket ws `*`-Allow does NOT loosen it; only an
//!      EXPLICIT per-tool ws rule does (the auditable admin write).
//!   3. **The runtime restriction:** activating the persona with a NON-default runtime fails at run
//!      start with a NAMED error, before any subprocess (in-house-only until the sandbox ships).
//!
//!   + **Workspace-isolation** and the **devkit hostile-input hardening** (a path-traversal id is
//!     rejected — the devkit verbs were built for a trusted Studio; an agent will fuzz them).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_persona_get, call_tool, clamp_to_preset, evaluate_policy, grant_skill,
    invoke_via_runtime, seed_core_skills, seed_personas, AgentError, AgentRuntime, AllowedTool,
    Effect, ErasedModel, Node, Policy, PolicyPreset, Rule, RunContext, RuntimeRegistry, Substrate,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use serde_json::json;

const PERSONA: &str = "builtin.extension-builder";

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

/// Seed personas + core skills and grant the extension-builder's pinned skills (so a run under it
/// isn't blocked by a fail-closed pin — that path is proven in agent_persona_test.rs). `granter` needs
/// `store:skill/**:write`.
async fn setup(node: &Arc<Node>, ws: &str, granter: &Principal) {
    seed_core_skills(&node.store, "0.1.0", 1).await.unwrap();
    seed_personas(&node.store).await.unwrap();
    let p = agent_persona_get(node, granter, ws, PERSONA).await.unwrap();
    for skill in &p.grounding_skills {
        let _ = grant_skill(&node.store, granter, ws, skill).await;
    }
}

/// The skill-write cap so a test can grant the persona's pins.
const SKILL_WRITE: &str = "store:skill/**:write";
const SKILL_READ: &str = "store:skill/**:read";
const INVOKE: &str = "mcp:agent.invoke:call";

// ---- a scripted external runtime (for the runtime-restriction test) -----------------------------

struct DummyExternal(String);
impl AgentRuntime for DummyExternal {
    fn id(&self) -> &str {
        &self.0
    }
    fn run<'a>(
        &'a self,
        _node: &'a Arc<Node>,
        _ctx: RunContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, AgentError>> + Send + 'a>>
    {
        // Should NEVER be reached for the extension-builder (the runtime restriction refuses first).
        Box::pin(async move { Ok("external ran".into()) })
    }
}

fn registry_with_external(id: &str) -> RuntimeRegistry {
    let default: Arc<dyn ErasedModel> =
        Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
            "x", 1,
        )])));
    let mut r = RuntimeRegistry::with_default(default);
    r.register(Arc::new(DummyExternal(id.into())));
    r
}

fn default_registry() -> RuntimeRegistry {
    let default: Arc<dyn ErasedModel> =
        Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
            "done", 1,
        )])));
    RuntimeRegistry::with_default(default)
}

// ================================================================================================
// 1. Admin-tier / capability-deny — a MEMBER caller can't scaffold or publish
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_caller_under_the_persona_is_denied_devkit_and_publish() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-deny";
    // A member with the invoke gate + skill-read (so grounding loads) but NONE of the devkit/ext caps.
    let member = principal(
        "user:mo",
        ws,
        &[INVOKE, SKILL_READ, "mcp:assets.load_skill:call"],
    );
    // An admin seeds + grants the pins.
    let admin = principal(
        "user:ada",
        ws,
        &[SKILL_WRITE, SKILL_READ, "mcp:agent.persona.get:call"],
    );
    setup(&node, ws, &admin).await;

    // The devkit/ext verbs deny at the wall for the member — regardless of the persona advertising them.
    // Driven through the full MCP dispatch (`call_tool`), the real path a run's proposed call takes.
    for tool in ["devkit.scaffold", "ext.publish"] {
        let err = call_tool(&node, &member, ws, tool, r#"{"id":"x","tier":"wasm"}"#)
            .await
            .expect_err("member denied");
        assert!(
            matches!(err, lb_mcp::ToolError::Denied),
            "{tool} is admin-tier — a member is denied at the wall (persona advertises, wall withholds)"
        );
    }
}

// ================================================================================================
// 2. The Ask floor (policy_preset) — as a resolved persona + as a merged policy
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_persona_carries_the_ask_preset_on_node_mutating_verbs() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.unwrap();
    let admin = principal("user:ada", "coding-preset", &["mcp:agent.persona.get:call"]);
    let p = agent_persona_get(&node, &admin, "coding-preset", PERSONA)
        .await
        .expect("resolves");
    let preset = p
        .policy_preset
        .expect("extension-builder carries a policy_preset");
    for gated in [
        "ext.publish",
        "ext.uninstall",
        "native.install",
        "native.reset",
    ] {
        assert!(
            preset.ask.iter().any(|t| t == gated),
            "{gated} is Ask-gated by the preset (node-mutating → human gate)"
        );
    }
    // The inner loop is NOT gated (supervision that prompts on every build gets turned off).
    for fluid in ["devkit.scaffold", "devkit.build", "devkit.inspect"] {
        assert!(
            !preset.ask.iter().any(|t| t == fluid) && !preset.deny.iter().any(|t| t == fluid),
            "{fluid} stays Allow — the edit/build inner loop is fluid"
        );
    }
}

/// Evaluate the ws policy then apply the persona floor clamp — the exact two-step the run loop does.
fn effective(ws_policy: &Policy, tool: &str, preset: &PolicyPreset) -> Effect {
    let ws_effect = evaluate_policy(ws_policy, tool, &json!({}));
    clamp_to_preset(ws_effect, tool, ws_policy, Some(preset))
}

#[test]
fn preset_floors_an_empty_ws_policy_to_ask() {
    // With no ws policy, a preset-Ask tool evaluates to Ask; the inner-loop tool stays Allow.
    let preset = PolicyPreset {
        ask: vec!["ext.publish".into()],
        deny: vec![],
    };
    assert_eq!(
        effective(&Policy::default(), "ext.publish", &preset),
        Effect::Ask,
        "the preset Ask holds over an empty ws policy"
    );
    assert_eq!(
        effective(&Policy::default(), "devkit.build", &preset),
        Effect::Allow,
        "a non-preset tool is unaffected (default-allow)"
    );
}

#[test]
fn a_blanket_ws_allow_does_not_loosen_the_floor_but_an_explicit_rule_does() {
    let preset = PolicyPreset {
        ask: vec!["ext.publish".into()],
        deny: vec![],
    };

    // (a) A blanket `*`-Allow ws policy must NOT loosen the floor — the clamp raises the specific tool
    //     back to Ask (a blanket rule is not an explicit decision about ext.publish).
    let blanket = Policy {
        rules: vec![Rule {
            tool: "*".into(),
            arg: None,
            effect: Effect::Allow,
        }],
    };
    assert_eq!(
        effective(&blanket, "ext.publish", &preset),
        Effect::Ask,
        "a blanket *-Allow does NOT silently loosen the preset floor (supervision holds)"
    );

    // (b) An EXPLICIT per-tool Allow (the auditable admin write) DOES loosen it — the admin has spoken.
    let explicit = Policy {
        rules: vec![Rule {
            tool: "ext.publish".into(),
            arg: None,
            effect: Effect::Allow,
        }],
    };
    assert_eq!(
        effective(&explicit, "ext.publish", &preset),
        Effect::Allow,
        "an explicit per-tool ws rule IS the admin write that loosens the floor"
    );

    // (c) A preset Deny is absolute; and the clamp never weakens a ws Deny on a preset-Ask tool.
    let deny_preset = PolicyPreset {
        ask: vec![],
        deny: vec!["native.install".into()],
    };
    assert_eq!(
        effective(&Policy::default(), "native.install", &deny_preset),
        Effect::Deny,
        "a preset Deny floor is absolute"
    );
}

// ================================================================================================
// 3. The runtime restriction — external runtime → named error at run start
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn activating_the_persona_with_an_external_runtime_fails_with_a_named_error() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-runtime";
    let admin = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            SKILL_WRITE,
            SKILL_READ,
            "mcp:agent.persona.get:call",
            "mcp:assets.load_skill:call",
        ],
    );
    setup(&node, ws, &admin).await;

    let registry = registry_with_external("some-external");
    let err = invoke_via_runtime(
        &node,
        &registry,
        Some("some-external"), // an external runtime
        Some(PERSONA),         // paired with the in-house-only persona
        &admin,
        &admin.caps().to_vec(),
        ws,
        "job-rt",
        "build me an extension",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("the pairing is refused");
    assert!(
        matches!(&err, AgentError::PersonaRuntime { persona, runtime, allowed }
            if persona == PERSONA && runtime == "some-external" && allowed == &vec!["default".to_string()]),
        "a named PersonaRuntime error, before any subprocess (got {err:?})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_persona_runs_fine_on_the_in_house_default_runtime() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-inhouse";
    let admin = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            SKILL_WRITE,
            SKILL_READ,
            "mcp:agent.persona.get:call",
            "mcp:assets.load_skill:call",
            "mcp:tools.catalog:call",
        ],
    );
    setup(&node, ws, &admin).await;

    // No runtime → default (in-house). The restriction allows `default`, so the run drives.
    let answer = invoke_via_runtime(
        &node,
        &default_registry(),
        None,
        Some(PERSONA),
        &admin,
        &admin.caps().to_vec(),
        ws,
        "job-ih",
        "build me an extension",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect("in-house run drives under the extension-builder");
    assert_eq!(answer, "done");
}

// ================================================================================================
// 4. Workspace-isolation — the persona is a built-in, readable everywhere, but a ws-B pick is walled
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_extension_builder_is_readable_in_every_workspace_but_seeded_once() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.unwrap();
    for ws in ["ws-a", "ws-b"] {
        let reader = principal("user:r", ws, &["mcp:agent.persona.get:call"]);
        let p = agent_persona_get(&node, &reader, ws, PERSONA)
            .await
            .expect("readable from every ws (built-in union)");
        assert!(p.builtin);
        assert_eq!(p.runtimes.as_deref(), Some(&["default".to_string()][..]));
    }
}

// ================================================================================================
// 5. Devkit hostile-input hardening — a path-traversal id is rejected (the agent WILL fuzz these)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_hostile_scaffold_id_is_rejected_not_a_filesystem_escape() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-fuzz";
    // An ADMIN caller (holds the devkit caps) so we get PAST the wall to the input validation — the
    // point is that even an authorized caller can't traverse the filesystem with a hostile id.
    let admin = principal("user:ada", ws, &["mcp:devkit.scaffold:call"]);

    for hostile in [
        "../escape",
        "..%2f..%2fetc",
        "foo/bar",
        "foo bar",
        "Foo", // uppercase — not kebab
        "-leading",
        "trailing-",
        "core-thing", // reserved prefix
    ] {
        let input = json!({ "id": hostile, "tier": "wasm", "features": [] }).to_string();
        let res = call_tool(&node, &admin, ws, "devkit.scaffold", &input).await;
        // A clean, typed rejection — `BadInput` (validation) or `Extension` (the devkit's own
        // "kebab-case ascii" guard); NEVER a filesystem traversal, a success, or a panic. The devkit's
        // `validate_id` (kebab-ascii, no `/`, no `..`, reserved-prefix reject) is the boundary.
        assert!(
            matches!(
                res,
                Err(lb_mcp::ToolError::BadInput(_)) | Err(lb_mcp::ToolError::Extension(_))
            ),
            "hostile scaffold id {hostile:?} must be a clean typed rejection, never a traversal \
             or a panic (got {res:?})"
        );
    }
}

// ================================================================================================
// 6. E2E — the persona reaches the REAL devkit, and its Ask-gate SUSPENDS a publish ("never on its own")
// ================================================================================================
// The full scaffold→build→publish→call-the-new-tool chain is proven end to end (with a real cargo
// build) in `devkit_e2e_test.rs` + `ext_publish_test.rs`. Here we prove the PERSONA-driven surface:
// (a) a real `devkit.scaffold` succeeds through the persona's granted devkit surface (no cargo build —
// that's the heavy test's job), and (b) a real run where the model proposes `ext.publish` SUSPENDS on
// the persona's Ask floor instead of publishing — the "100% coding, but never on its own" guarantee.

use lb_jobs::{load as load_job, JobStatus};

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_scaffold_works_through_the_persona_devkit_surface() {
    // The persona advertises `devkit.scaffold`; an admin caller reaches the REAL devkit verb and a real
    // extension tree lands on disk. (Scaffold only — the cargo build is the heavy e2e test's concern.)
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-e2e-scaffold";
    let admin = principal("user:ada", ws, &["mcp:devkit.scaffold:call"]);

    let root = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("persona-ext-e2e");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    // Drive the REAL host verb (the same one the persona's run would call), rooted at our scratch dir.
    let report = lb_host::devkit_scaffold(
        &admin,
        ws,
        Some(&root),
        &lb_devkit::ScaffoldRequest {
            id: "energy-heatmap".into(),
            tier: lb_devkit::Tier::Wasm,
            features: vec![lb_devkit::Feature::SeriesRead],
        },
    )
    .expect("scaffold succeeds through the persona's devkit surface");

    // A real extension tree exists — the manifest + a source file the persona would then edit + build.
    assert!(
        report.path.join("extension.toml").exists(),
        "manifest scaffolded"
    );
    assert!(report.path.join("build.sh").exists(), "build.sh scaffolded");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_publish_proposed_under_the_persona_suspends_for_a_human_it_never_publishes_on_its_own() {
    // THE "never on its own" proof: a real run under the extension-builder persona, where the model
    // proposes `ext.publish`. The persona's Ask floor turns that into a durable SUSPENSION (an
    // agent_decision awaiting a human) — the publish does NOT happen until a human `agent.decide`s.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "coding-e2e-ask";
    // The caller HOLDS ext.publish (so the gate is the persona's Ask policy, not a wall-deny) + the
    // skill-read/invoke/publish caps. The run is on the in-house default (the persona allows it).
    let admin = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            SKILL_WRITE,
            SKILL_READ,
            "mcp:agent.persona.get:call",
            "mcp:assets.load_skill:call",
            "mcp:ext.publish:call",
        ],
    );
    setup(&node, ws, &admin).await;

    // A model that proposes exactly one call: ext.publish. The Ask floor must intercept it.
    let publishing_model: Arc<dyn ErasedModel> = Arc::new(AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll publish the extension.",
            vec![lb_role_ai_gateway::ToolCall {
                id: "c1".into(),
                name: "ext.publish".into(),
                input: r#"{"artifact":"x"}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("published", 5),
    ])));
    let registry = RuntimeRegistry::with_default(publishing_model);

    let job_id = "job-ask";
    invoke_via_runtime(
        &node,
        &registry,
        None, // in-house default (persona allows it)
        Some(PERSONA),
        &admin,
        &admin.caps().to_vec(),
        ws,
        job_id,
        "publish the extension you built",
        Substrate::default(),
        None,
        &[AllowedTool {
            name: "ext.publish".into(),
            description: "publish".into(),
            input_schema: None,
        }],
        1,
    )
    .await
    .expect("run drives to a suspension");

    // The run is durably SUSPENDED on the Ask'd publish — it did NOT publish on its own.
    let job = load_job(&node.store, ws, job_id)
        .await
        .expect("job load")
        .expect("job exists");
    assert_eq!(
        job.status,
        JobStatus::Suspended,
        "the persona's Ask floor SUSPENDED the run on ext.publish — never on its own"
    );
    assert!(
        job.events().any(|e| matches!(
            e,
            lb_jobs::TranscriptEvent::SuspensionOpened { tool_call_id, .. } if tool_call_id == "c1"
        )),
        "a durable SuspensionOpened for the publish call awaits a human agent.decide"
    );
}
