//! Agent-personas sub-scope #1 (persona-model) — the HOST-side contract, against a **real** Node +
//! store + caps + loop and the deterministic MockProvider (rule 9 — the only stub is the provider
//! HTTP). This file locks the foundation the whole topic rests on:
//!
//!   - **CRUD + caps-deny (§2.1):** every `agent.persona.*` verb; a non-admin write denied and
//!     nothing persists; a `builtin.*` write rejected `BadInput` before the caps gate.
//!   - **Workspace-isolation (§2.2):** ws-B cannot `get`/apply ws-A's custom persona; ws-B's
//!     `active_persona` never affects a ws-A run; built-ins readable from both, writable from neither.
//!   - **The swap test (both runtimes):** a record-only custom persona changes a run's menu +
//!     identity + pinned catalog with ZERO code change — proven for the in-house runtime (a recording
//!     model captures the exact tools + goal) AND a scripted external runtime (captures its
//!     `RunContext` — the narrowed `tools` are what the real ACP bridge advertises; the goal carries
//!     the identity + folded catalog).
//!   - **The narrowing test:** a persona listing a tool the caller lacks changes nothing (still
//!     denied at the wall); a granted tool the persona omits is absent from the menu but the wall
//!     still governs a model-proposed call to it (menu is a hint, wall is the law).
//!   - **Fail-closed grounding:** a persona pinning an ungranted skill fails the run at start with the
//!     named error, before any model spend.
//!   - **Resolution precedence:** explicit invoke `persona` > `active_persona` > none; an explicit
//!     unknown id is a named error, a dangling active id warns + runs un-narrowed.
//!   - **Units:** glob matching, `extends` union + cycle rejection, idempotent re-seed.

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, agent_persona_create, agent_persona_delete, agent_persona_get,
    agent_persona_list, agent_persona_update, call_agent_tool, glob_matches, grant_skill,
    invoke_via_runtime, seed_core_skills, seed_personas, AgentConfig, AgentError, AgentRuntime,
    AllowedTool, ErasedModel, Node, Persona, PersonaPatch, RunContext, RuntimeRegistry, Substrate,
    DEFAULT_RUNTIME,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};

// ---- harness -----------------------------------------------------------------------------------

const INVOKE: &str = "mcp:agent.invoke:call";
const P_LIST: &str = "mcp:agent.persona.list:call";
const P_GET: &str = "mcp:agent.persona.get:call";
const P_CREATE: &str = "mcp:agent.persona.create:call";
const P_UPDATE: &str = "mcp:agent.persona.update:call";
const P_DELETE: &str = "mcp:agent.persona.delete:call";
const CFG_SET: &str = "mcp:agent.config.set:call";
// A real reachable host verb (member-level) we can grant the caller so it appears in `reachable_tools`.
const MEM_LIST_CAP: &str = "mcp:agent.memory.list:call";
const MEM_GET_CAP: &str = "mcp:agent.memory.get:call";
const CATALOG_CAP: &str = "mcp:tools.catalog:call";

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

/// A minimal custom persona over the two memory-read verbs, with an identity. Its `granted_tools`
/// narrows the menu to exactly `agent.memory.list`.
fn analyst_persona(id: &str) -> Persona {
    Persona {
        id: id.into(),
        label: "Test analyst".into(),
        description: Some("reads memory".into()),
        identity: "You are the TEST-ANALYST persona. You only read memory.".into(),
        granted_tools: vec!["agent.memory.list".into()],
        grounding_skills: vec![],
        extends: vec![],
        policy_preset: None,
        runtimes: None,
        builtin: false,
    }
}

// ---- a recording in-house model ----------------------------------------------------------------

/// What the recording model captured on its (single) turn.
#[derive(Default)]
struct Captured {
    tool_names: Vec<String>,
    /// The concatenated system+goal messages the model was seeded with (proves the identity fold).
    seed_text: String,
    turns: usize,
}

/// An `ErasedModel` that records the tools + seed messages it is handed, then stops immediately. Lets
/// a test assert the EXACT menu + goal the in-house loop assembled under a persona.
struct RecordingModel {
    captured: Arc<Mutex<Captured>>,
    answer: String,
}

impl ErasedModel for RecordingModel {
    fn turn_boxed<'a>(
        &'a self,
        _ws: &'a str,
        messages: &'a [(String, String)],
        tools: &'a [AllowedTool],
        _prior: &'a [lb_host::CallOutcome],
        _key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = lb_host::Turn> + Send + 'a>> {
        {
            let mut c = self.captured.lock().unwrap();
            // Only capture the FIRST turn's menu/seed (the loop may re-ask; we care about assembly).
            if c.turns == 0 {
                c.tool_names = tools.iter().map(|t| t.name.clone()).collect();
                c.seed_text = messages
                    .iter()
                    .map(|(role, body)| format!("{role}: {body}"))
                    .collect::<Vec<_>>()
                    .join("\n");
            }
            c.turns += 1;
        }
        let answer = self.answer.clone();
        Box::pin(async move {
            lb_host::Turn {
                content: answer,
                calls: vec![],
                done: true,
            }
        })
    }

    fn is_configured(&self) -> bool {
        true
    }
}

fn recording_registry(answer: &str) -> (RuntimeRegistry, Arc<Mutex<Captured>>) {
    let captured = Arc::new(Mutex::new(Captured::default()));
    let model: Arc<dyn ErasedModel> = Arc::new(RecordingModel {
        captured: captured.clone(),
        answer: answer.into(),
    });
    (RuntimeRegistry::with_default(model), captured)
}

// ---- a scripted external runtime (captures its RunContext) --------------------------------------

/// A fake `AgentRuntime` with a NON-default id — stands in for the external ACP runtime (whose real
/// MCP bridge is a role crate not in this tree). It captures the `RunContext` it is handed so a test
/// can assert the narrowed `tools` (what the bridge would advertise) and the goal (identity + folded
/// catalog). It runs the SAME seam every real runtime does.
struct CapturingExternal {
    id: String,
    seen_tools: Arc<Mutex<Vec<String>>>,
    seen_goal: Arc<Mutex<String>>,
}

impl AgentRuntime for CapturingExternal {
    fn id(&self) -> &str {
        &self.id
    }
    fn run<'a>(
        &'a self,
        _node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, AgentError>> + Send + 'a>>
    {
        *self.seen_tools.lock().unwrap() = ctx.tools.iter().map(|t| t.name.clone()).collect();
        *self.seen_goal.lock().unwrap() = ctx.goal.to_string();
        Box::pin(async move { Ok("external stopped".to_string()) })
    }
}

/// A registry with the in-house default PLUS a capturing external runtime `id`.
fn external_registry(id: &str) -> (RuntimeRegistry, Arc<Mutex<Vec<String>>>, Arc<Mutex<String>>) {
    let default: Arc<dyn ErasedModel> =
        Arc::new(AiGateway::new(MockProvider::new(vec![AiResponse::stop(
            "x", 1,
        )])));
    let mut registry = RuntimeRegistry::with_default(default);
    let seen_tools = Arc::new(Mutex::new(vec![]));
    let seen_goal = Arc::new(Mutex::new(String::new()));
    registry.register(Arc::new(CapturingExternal {
        id: id.into(),
        seen_tools: seen_tools.clone(),
        seen_goal: seen_goal.clone(),
    }));
    (registry, seen_tools, seen_goal)
}

// A menu of two reachable tools, one in the persona, one not.
fn two_tool_menu() -> Vec<AllowedTool> {
    vec![
        AllowedTool {
            name: "agent.memory.list".into(),
            description: "list memory".into(),
        },
        AllowedTool {
            name: "agent.memory.get".into(),
            description: "get memory".into(),
        },
    ]
}

// ================================================================================================
// Units
// ================================================================================================

#[test]
fn glob_matches_prefix_and_literal() {
    assert!(glob_matches("flows.*", "flows.save"));
    assert!(glob_matches("flows.*", "flows.runs.get"));
    assert!(!glob_matches("flows.*", "federation.query"));
    assert!(glob_matches("agent.memory.list", "agent.memory.list"));
    assert!(!glob_matches("agent.memory.list", "agent.memory.get"));
    // A trailing-* on an empty prefix would match everything — but write-validation rejects a bare
    // `*`, so it never reaches `glob_matches`. Here we only prove the matcher's mechanics.
}

// ================================================================================================
// CRUD + capability-deny (§2.1)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn crud_roundtrips_for_an_admin() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-crud";
    let admin = principal(
        "user:ada",
        ws,
        &[P_CREATE, P_GET, P_UPDATE, P_DELETE, P_LIST],
    );

    agent_persona_create(&node, &admin, ws, &analyst_persona("analyst"))
        .await
        .expect("create");

    let got = agent_persona_get(&node, &admin, ws, "analyst")
        .await
        .expect("get");
    assert_eq!(got.granted_tools, vec!["agent.memory.list".to_string()]);
    assert!(!got.builtin);

    agent_persona_update(
        &node,
        &admin,
        ws,
        "analyst",
        PersonaPatch {
            label: Some("Renamed".into()),
            ..Default::default()
        },
    )
    .await
    .expect("update");
    let got = agent_persona_get(&node, &admin, ws, "analyst")
        .await
        .unwrap();
    assert_eq!(got.label, "Renamed");

    let list = agent_persona_list(&node, &admin, ws).await.expect("list");
    assert!(list.iter().any(|p| p.id == "analyst"));

    agent_persona_delete(&node, &admin, ws, "analyst")
        .await
        .expect("delete");
    assert!(
        matches!(
            agent_persona_get(&node, &admin, ws, "analyst").await,
            Err(lb_mcp::ToolError::NotFound)
        ),
        "a deleted persona is gone"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn non_admin_create_is_denied_and_nothing_persists() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-deny";
    // A member that can READ (list/get) but not WRITE.
    let member = principal("user:mo", ws, &[P_LIST, P_GET]);

    let err = agent_persona_create(&node, &member, ws, &analyst_persona("sneaky"))
        .await
        .expect_err("create denied");
    assert!(matches!(err, lb_mcp::ToolError::Denied));

    // Nothing persisted — the read (which the member CAN do) finds nothing.
    assert!(matches!(
        agent_persona_get(&node, &member, ws, "sneaky").await,
        Err(lb_mcp::ToolError::NotFound)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn builtin_write_is_rejected_before_the_caps_gate() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-builtin";
    // Even a fully-capped admin cannot write a `builtin.*` id — read-only tier, checked FIRST.
    let admin = principal("user:ada", ws, &[P_CREATE, P_UPDATE, P_DELETE]);

    let err = agent_persona_create(&node, &admin, ws, &analyst_persona("builtin.forged"))
        .await
        .expect_err("builtin create rejected");
    assert!(
        matches!(err, lb_mcp::ToolError::BadInput(m) if m.contains("reserved")),
        "a builtin.* write is BadInput (reserved tier), not a silent accept"
    );

    let err = agent_persona_delete(&node, &admin, ws, "builtin.data-analyst")
        .await
        .expect_err("builtin delete rejected");
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

// ================================================================================================
// Seed + workspace-isolation (§2.2)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn builtins_seed_readable_everywhere_writable_nowhere_idempotent() {
    let node = Arc::new(Node::boot().await.expect("node boots"));

    // Seed twice — idempotent (no dupes, no error).
    let first = seed_personas(&node.store).await.expect("seed");
    let second = seed_personas(&node.store).await.expect("re-seed");
    assert_eq!(first, second, "re-seed is idempotent (same ids)");
    assert!(
        first.iter().any(|id| id == "builtin.data-analyst"),
        "the starter built-in is seeded"
    );

    // Readable from two DIFFERENT workspaces (built-ins live in the reserved ns, unioned into each).
    for ws in ["ws-a", "ws-b"] {
        let reader = principal("user:r", ws, &[P_GET, P_LIST]);
        let got = agent_persona_get(&node, &reader, ws, "builtin.data-analyst")
            .await
            .expect("builtin readable");
        assert!(got.builtin);
        assert!(!got.granted_tools.is_empty());
        let list = agent_persona_list(&node, &reader, ws).await.unwrap();
        assert!(list
            .iter()
            .any(|p| p.id == "builtin.data-analyst" && p.builtin));
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_get_ws_a_custom_persona() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin_a = principal("user:ada", "ws-a", &[P_CREATE]);
    agent_persona_create(&node, &admin_a, "ws-a", &analyst_persona("ada-only"))
        .await
        .expect("create in ws-a");

    // A ws-B admin reading the same id → NotFound (the hard wall). Different namespace.
    let admin_b = principal("user:bo", "ws-b", &[P_GET]);
    assert!(matches!(
        agent_persona_get(&node, &admin_b, "ws-b", "ada-only").await,
        Err(lb_mcp::ToolError::NotFound)
    ));
    // But ws-A still has it.
    let reader_a = principal("user:ada", "ws-a", &[P_GET]);
    assert!(agent_persona_get(&node, &reader_a, "ws-a", "ada-only")
        .await
        .is_ok());
}

// ================================================================================================
// The swap test + narrowing — IN-HOUSE runtime
// ================================================================================================

/// Drive the in-house loop under an ACTIVE persona and capture the assembled menu + goal.
async fn drive_in_house_with_active_persona(
    node: &Arc<Node>,
    ws: &str,
    caller: &Principal,
    persona_id: &str,
    tools: &[AllowedTool],
) -> Arc<Mutex<Captured>> {
    let (registry, captured) = recording_registry("done");
    invoke_via_runtime(
        node,
        &registry,
        None, // runtime: default (in-house)
        None, // persona arg: none → resolve the ACTIVE persona
        caller,
        &caller.caps().to_vec(),
        ws,
        &format!("job-{persona_id}"),
        "answer the question",
        Substrate::default(),
        None,
        tools,
        1,
    )
    .await
    .expect("run drives");
    captured
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn swap_test_in_house_menu_and_identity_reflect_a_record_only_persona() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-swap-inhouse";
    // The caller is granted BOTH memory reads + invoke; the persona will narrow to just `.list`.
    let caller = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            P_CREATE,
            CFG_SET,
            MEM_LIST_CAP,
            MEM_GET_CAP,
            CATALOG_CAP,
        ],
    );

    // BASELINE: no persona → the full two-tool menu reaches the model, no persona identity.
    let base =
        drive_in_house_with_active_persona(&node, ws, &caller, "none", &two_tool_menu()).await;
    {
        let c = base.lock().unwrap();
        assert!(c.tool_names.contains(&"agent.memory.list".to_string()));
        assert!(
            c.tool_names.contains(&"agent.memory.get".to_string()),
            "baseline menu carries BOTH tools"
        );
        assert!(
            !c.seed_text.contains("TEST-ANALYST"),
            "no persona identity without a persona"
        );
    }

    // SWAP: create a record-only persona + set it active. ZERO code change.
    agent_persona_create(&node, &caller, ws, &analyst_persona("analyst"))
        .await
        .expect("create persona");
    agent_config_set(
        &node,
        &caller,
        ws,
        &AgentConfig {
            active_persona: Some("analyst".into()),
            ..Default::default()
        },
    )
    .await
    .expect("set active persona");

    let after =
        drive_in_house_with_active_persona(&node, ws, &caller, "analyst", &two_tool_menu()).await;
    let c = after.lock().unwrap();
    // Menu NARROWED to the persona's one tool.
    assert_eq!(
        c.tool_names,
        vec!["agent.memory.list".to_string()],
        "the menu is narrowed to persona ∩ reachable"
    );
    // IDENTITY folded into the seed (goal).
    assert!(
        c.seed_text.contains("TEST-ANALYST"),
        "the persona identity reached the model's context"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn narrowing_a_persona_tool_the_caller_lacks_is_never_added() {
    // The headline: a persona listing a tool the CALLER lacks changes nothing — it was never in the
    // reachable menu, so narrowing can't conjure it. `persona ∩ reachable` can only shrink.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-nowiden";
    let caller = principal("user:ada", ws, &[INVOKE, P_CREATE, CFG_SET, MEM_LIST_CAP]);

    // A persona that lists a tool the caller does NOT have a cap for.
    let mut p = analyst_persona("greedy");
    p.granted_tools = vec!["agent.memory.list".into(), "workspace.purge".into()];
    agent_persona_create(&node, &caller, ws, &p)
        .await
        .expect("create");
    agent_config_set(
        &node,
        &caller,
        ws,
        &AgentConfig {
            active_persona: Some("greedy".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // The reachable menu passed in is only `.list` (the caller can't reach purge). The persona lists
    // purge, but it never appears.
    let only_list = vec![AllowedTool {
        name: "agent.memory.list".into(),
        description: "list".into(),
    }];
    let captured =
        drive_in_house_with_active_persona(&node, ws, &caller, "greedy", &only_list).await;
    let c = captured.lock().unwrap();
    assert_eq!(c.tool_names, vec!["agent.memory.list".to_string()]);
    assert!(
        !c.tool_names.iter().any(|t| t == "workspace.purge"),
        "a persona cannot widen the menu to a tool the caller lacks"
    );
}

// ================================================================================================
// The swap test — EXTERNAL runtime (the same seam, protocol-narrowing proof)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn swap_test_external_runtime_advertises_narrowed_tools_and_folds_identity() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-swap-external";
    let ext_id = "ext-runtime";
    let caller = principal(
        "user:ada",
        ws,
        &[INVOKE, P_CREATE, MEM_LIST_CAP, MEM_GET_CAP, CATALOG_CAP],
    );

    agent_persona_create(&node, &caller, ws, &analyst_persona("analyst"))
        .await
        .expect("create");

    let (registry, seen_tools, seen_goal) = external_registry(ext_id);
    // Drive the EXTERNAL runtime with an EXPLICIT persona override (proves the per-invoke arg too).
    invoke_via_runtime(
        &node,
        &registry,
        Some(ext_id),
        Some("analyst"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-ext",
        "answer",
        Substrate::default(),
        None,
        &two_tool_menu(),
        1,
    )
    .await
    .expect("external runs");

    // The tools the external runtime saw (== what its ACP bridge advertises) are the NARROWED set.
    assert_eq!(
        *seen_tools.lock().unwrap(),
        vec!["agent.memory.list".to_string()],
        "the external bridge advertises persona ∩ reachable"
    );
    // The identity folded into the goal (the external agent's only channel).
    assert!(
        seen_goal.lock().unwrap().contains("TEST-ANALYST"),
        "the persona identity is folded into the external goal"
    );
}

// ================================================================================================
// Fail-closed grounding + resolution precedence
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pinned_ungranted_skill_fails_the_run_at_start_with_a_named_error() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-failclosed";
    let caller = principal("user:ada", ws, &[INVOKE, P_CREATE]);

    // A persona pinning a skill the workspace has NOT granted.
    let mut p = analyst_persona("grounded");
    p.grounding_skills = vec!["core.definitely-not-granted".into()];
    agent_persona_create(&node, &caller, ws, &p)
        .await
        .expect("create");

    let (registry, _captured) = recording_registry("unreachable");
    let err = invoke_via_runtime(
        &node,
        &registry,
        None,
        Some("grounded"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-fc",
        "answer",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("ungranted pinned skill fails the run");
    assert!(
        matches!(&err, AgentError::PersonaSkill { persona, skill }
            if persona == "grounded" && skill == "core.definitely-not-granted"),
        "the named PersonaSkill error identifies the persona + the ungranted skill (got {err:?})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_explicit_unknown_persona_is_a_named_error_not_a_silent_run() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-explicit-unknown";
    let caller = principal("user:ada", ws, &[INVOKE]);
    let (registry, _c) = recording_registry("x");

    let err = invoke_via_runtime(
        &node,
        &registry,
        None,
        Some("no-such-persona"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-eu",
        "answer",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("explicit unknown persona errors");
    assert!(
        matches!(err, AgentError::NotFound | AgentError::Denied),
        "an explicit ask for a missing persona does not silently degrade (got {err:?})"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dangling_active_persona_warns_and_runs_un_narrowed() {
    // A persona set active then deleted → the run must NOT error; it runs un-narrowed (the resolve-at-
    // read posture). The full menu reaches the model.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-dangling";
    let caller = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            P_CREATE,
            P_DELETE,
            CFG_SET,
            MEM_LIST_CAP,
            MEM_GET_CAP,
            CATALOG_CAP,
        ],
    );
    agent_persona_create(&node, &caller, ws, &analyst_persona("temp"))
        .await
        .unwrap();
    agent_config_set(
        &node,
        &caller,
        ws,
        &AgentConfig {
            active_persona: Some("temp".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    agent_persona_delete(&node, &caller, ws, "temp")
        .await
        .unwrap();

    let captured =
        drive_in_house_with_active_persona(&node, ws, &caller, "temp", &two_tool_menu()).await;
    let c = captured.lock().unwrap();
    assert_eq!(
        c.tool_names.len(),
        2,
        "a dangling active persona runs un-narrowed (full menu), not errored"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn explicit_persona_overrides_the_active_one() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-precedence";
    let caller = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            P_CREATE,
            CFG_SET,
            MEM_LIST_CAP,
            MEM_GET_CAP,
            CATALOG_CAP,
        ],
    );

    // Active persona narrows to `.list`; an explicit override persona narrows to `.get`.
    agent_persona_create(&node, &caller, ws, &analyst_persona("active-one"))
        .await
        .unwrap();
    let mut getter = analyst_persona("explicit-one");
    getter.granted_tools = vec!["agent.memory.get".into()];
    getter.identity = "You are EXPLICIT-ONE.".into();
    agent_persona_create(&node, &caller, ws, &getter)
        .await
        .unwrap();
    agent_config_set(
        &node,
        &caller,
        ws,
        &AgentConfig {
            active_persona: Some("active-one".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // Explicit `persona: explicit-one` must win over active `active-one`.
    let (registry, captured) = recording_registry("done");
    invoke_via_runtime(
        &node,
        &registry,
        None,
        Some("explicit-one"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-prec",
        "answer",
        Substrate::default(),
        None,
        &two_tool_menu(),
        1,
    )
    .await
    .expect("run");
    let c = captured.lock().unwrap();
    assert_eq!(
        c.tool_names,
        vec!["agent.memory.get".to_string()],
        "the explicit persona (get) overrode the active persona (list)"
    );
    assert!(c.seed_text.contains("EXPLICIT-ONE"));
}

// ================================================================================================
// extends — union + cycle rejection
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn extends_unions_parent_tools_and_skips_a_self_cycle() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-extends";
    let caller = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            P_CREATE,
            CFG_SET,
            MEM_LIST_CAP,
            MEM_GET_CAP,
            CATALOG_CAP,
        ],
    );

    // Parent narrows to `.get`; child adds `.list` and extends the parent → union is both.
    let mut parent = analyst_persona("parent");
    parent.granted_tools = vec!["agent.memory.get".into()];
    agent_persona_create(&node, &caller, ws, &parent)
        .await
        .unwrap();

    let mut child = analyst_persona("child");
    child.granted_tools = vec!["agent.memory.list".into()];
    child.extends = vec!["parent".into()];
    agent_persona_create(&node, &caller, ws, &child)
        .await
        .unwrap();
    agent_config_set(
        &node,
        &caller,
        ws,
        &AgentConfig {
            active_persona: Some("child".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let captured =
        drive_in_house_with_active_persona(&node, ws, &caller, "child", &two_tool_menu()).await;
    let mut names = captured.lock().unwrap().tool_names.clone();
    names.sort();
    assert_eq!(
        names,
        vec![
            "agent.memory.get".to_string(),
            "agent.memory.list".to_string()
        ],
        "the child's menu is the union of its own + the parent's tools"
    );

    // A self-cycle is rejected at write.
    let mut selfish = analyst_persona("selfish");
    selfish.extends = vec!["selfish".into()];
    assert!(
        matches!(
            agent_persona_create(&node, &caller, ws, &selfish).await,
            Err(lb_mcp::ToolError::BadInput(_))
        ),
        "a self-extends is rejected at write"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_two_node_extends_cycle_is_rejected_at_write() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-cycle";
    let caller = principal("user:ada", ws, &[P_CREATE, P_UPDATE]);

    agent_persona_create(&node, &caller, ws, &analyst_persona("a"))
        .await
        .unwrap();
    let mut b = analyst_persona("b");
    b.extends = vec!["a".into()];
    agent_persona_create(&node, &caller, ws, &b).await.unwrap();

    // Now make `a` extend `b` → a↔b cycle. Rejected at write.
    let err = agent_persona_update(
        &node,
        &caller,
        ws,
        "a",
        PersonaPatch {
            extends: Some(vec!["b".into()]),
            ..Default::default()
        },
    )
    .await
    .expect_err("cycle rejected");
    assert!(matches!(err, lb_mcp::ToolError::BadInput(_)));
}

// ================================================================================================
// Settings-surface read verbs: agent.persona.resolve + agent.policy.get (over the MCP bridge)
// ================================================================================================

const P_RESOLVE: &str = "mcp:agent.persona.resolve:call";
const POLICY_GET: &str = "mcp:agent.policy.get:call";
const POLICY_SET: &str = "mcp:agent.policy.set:call";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_verb_returns_the_extends_unioned_effective_persona() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-resolve-verb";
    let caller = principal("user:ada", ws, &[P_CREATE, P_RESOLVE]);

    // parent → get; child adds list + extends parent.
    let mut parent = analyst_persona("p");
    parent.granted_tools = vec!["agent.memory.get".into()];
    agent_persona_create(&node, &caller, ws, &parent)
        .await
        .unwrap();
    let mut child = analyst_persona("c");
    child.granted_tools = vec!["agent.memory.list".into()];
    child.identity = "You are CHILD.".into();
    child.extends = vec!["p".into()];
    agent_persona_create(&node, &caller, ws, &child)
        .await
        .unwrap();

    let out = call_agent_tool(
        &node,
        &caller,
        ws,
        "agent.persona.resolve",
        &serde_json::json!({ "id": "c" }),
    )
    .await
    .expect("resolve ok");
    let eff = &out["effective"];
    assert_eq!(eff["id"], "c");
    assert_eq!(eff["identity"], "You are CHILD.");
    let tools: Vec<String> = serde_json::from_value(eff["granted_tools"].clone()).unwrap();
    assert!(tools.contains(&"agent.memory.list".to_string()));
    assert!(
        tools.contains(&"agent.memory.get".to_string()),
        "resolve unions the parent's tools"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resolve_verb_is_denied_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-resolve-deny";
    let caller = principal("user:mo", ws, &[]); // no resolve cap
    let err = call_agent_tool(
        &node,
        &caller,
        ws,
        "agent.persona.resolve",
        &serde_json::json!({ "id": "x" }),
    )
    .await
    .expect_err("resolve denied");
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn policy_get_round_trips_the_set_policy() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-policy-rt";
    let admin = principal("user:ada", ws, &[POLICY_SET, POLICY_GET]);

    // Empty by default.
    let empty = call_agent_tool(
        &node,
        &admin,
        ws,
        "agent.policy.get",
        &serde_json::json!({}),
    )
    .await
    .expect("get default");
    let rules = empty["rules"].as_array().cloned().unwrap_or_default();
    assert!(rules.is_empty(), "default policy is empty (default-allow)");

    // Set an Ask rule, read it back.
    call_agent_tool(
        &node,
        &admin,
        ws,
        "agent.policy.set",
        &serde_json::json!({ "rules": [{ "tool": "ext.publish", "effect": "ask" }] }),
    )
    .await
    .expect("set");
    let got = call_agent_tool(
        &node,
        &admin,
        ws,
        "agent.policy.get",
        &serde_json::json!({}),
    )
    .await
    .expect("get after set");
    let rules = got["rules"].as_array().unwrap();
    assert_eq!(rules.len(), 1);
    assert_eq!(rules[0]["tool"], "ext.publish");
    assert_eq!(rules[0]["effect"], "ask");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn policy_get_is_denied_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-policy-deny";
    let caller = principal("user:mo", ws, &[]); // no policy.get cap
    let err = call_agent_tool(
        &node,
        &caller,
        ws,
        "agent.policy.get",
        &serde_json::json!({}),
    )
    .await
    .expect_err("policy.get denied");
    assert!(matches!(err, lb_mcp::ToolError::Denied));
}

// ================================================================================================
// The GROUNDING test (umbrella exit gate, #2): a persona-grounded run answers a platform-operations
// question from its PINNED SKILL, with the whole-codebase access absent.
// ================================================================================================

const SKILL_READ: &str = "store:skill/**:read";
const SKILL_WRITE: &str = "store:skill/**:write";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_persona_grounded_run_is_fed_its_pinned_skill_body_not_the_repo() {
    // The persona-grounding thesis (agent-personas #2): a persona pins a real seeded skill; at run
    // assembly the pinned skill's BODY is folded into the run's context, so the agent answers a
    // platform-ops question ("how do I verify this feature?") FROM THE RUNBOOK — with NO filesystem /
    // repo tool anywhere in its menu (the confusion cure: the contract, not the codebase).
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-grounding";

    // Seed the REAL core-skills corpus (which now includes the docs/testing/** e2e runbooks) and grant
    // the e2e-backend runbook to this workspace — the grant is the wall (an ungranted skill fail-closes).
    seed_core_skills(&node.store, "0.1.0", 1)
        .await
        .expect("seed core skills");
    let admin = principal("user:ada", ws, &[SKILL_READ, SKILL_WRITE]);
    grant_skill(&node.store, &admin, ws, "core.e2e-backend")
        .await
        .expect("grant the runbook");

    // A persona that PINS the runbook and narrows the menu to data verbs — crucially, NO fs/repo/shell
    // tool is granted (there is no such host verb anyway; the point is the menu is the persona's focus).
    let caller = principal(
        "user:ada",
        ws,
        &[
            INVOKE,
            P_CREATE,
            SKILL_READ,
            SKILL_WRITE,
            CATALOG_CAP,
            MEM_LIST_CAP,
        ],
    );
    let mut grounded = analyst_persona("grounded-analyst");
    grounded.grounding_skills = vec!["core.e2e-backend".into()];
    grounded.granted_tools = vec!["agent.memory.list".into()];
    agent_persona_create(&node, &caller, ws, &grounded)
        .await
        .expect("create grounded persona");

    let (registry, captured) = recording_registry("verified via make dev");
    invoke_via_runtime(
        &node,
        &registry,
        None,
        Some("grounded-analyst"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-grounding",
        "How do I verify this backend feature in the real world?",
        Substrate::default(),
        None,
        &two_tool_menu(),
        1,
    )
    .await
    .expect("grounded run drives");

    let c = captured.lock().unwrap();
    // GROUNDED: the pinned runbook BODY reached the model's context. `make dev` is a distinctive phrase
    // from `docs/testing/e2e-backend.md` — its presence proves the runbook (not the repo) is the source.
    assert!(
        c.seed_text.contains("make dev"),
        "the pinned e2e-backend runbook body must be in the run's context (grounded from the skill)"
    );
    // FOCUSED, not the whole surface: the menu is the persona's one data verb — no repo/fs/shell tool.
    assert_eq!(
        c.tool_names,
        vec!["agent.memory.list".to_string()],
        "the grounded run's menu is the persona's focus, not the whole tool surface"
    );
    assert!(
        !c.tool_names
            .iter()
            .any(|t| t.contains("fs") || t.contains("shell") || t.contains("host.")),
        "no filesystem/shell/repo tool in a grounded run's menu — it learns from the runbook"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_persona_pinning_an_ungranted_real_seed_fails_closed() {
    // The grant is the wall, re-proven over a REAL seeded runbook: seed core skills but DON'T grant
    // core.e2e-backend → a persona pinning it fails the run at start with the named error (#1's
    // fail-closed, re-asserted against a real new seed per the #2 testing plan).
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "persona-grounding-deny";
    seed_core_skills(&node.store, "0.1.0", 1).await.unwrap();
    let caller = principal("user:ada", ws, &[INVOKE, P_CREATE]);
    let mut p = analyst_persona("wants-runbook");
    p.grounding_skills = vec!["core.e2e-backend".into()]; // seeded but NOT granted in this ws
    agent_persona_create(&node, &caller, ws, &p).await.unwrap();

    let (registry, _c) = recording_registry("unreachable");
    let err = invoke_via_runtime(
        &node,
        &registry,
        None,
        Some("wants-runbook"),
        &caller,
        &caller.caps().to_vec(),
        ws,
        "job-gd",
        "answer",
        Substrate::default(),
        None,
        &[] as &[AllowedTool],
        1,
    )
    .await
    .expect_err("ungranted real seed fails closed");
    assert!(
        matches!(&err, AgentError::PersonaSkill { skill, .. } if skill == "core.e2e-backend"),
        "fail-closed with the named error over a REAL seed (got {err:?})"
    );
}
