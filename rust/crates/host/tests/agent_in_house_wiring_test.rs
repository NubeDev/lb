//! Default-agent-wiring scope — finishing the in-house default agent so it can use the platform's own
//! tools through the ONE capability wall (`call_tool`), wired to a real model at boot.
//!
//! **Rule 9: everything real** — the `mem://` store, the bus, caps, the gateway, and the loop are the
//! real code. The ONLY permitted fake is the model **provider HTTP** (`MockProvider`, behind the
//! `Provider` trait), which scripts the model's turns so a test drives an exact path with no network.
//!
//! What this proves (the scope's Testing plan):
//!   - **The headline wiring** — a real `AiGateway<MockProvider>` installed as the in-house model runs
//!     the loop; the scripted model proposes a **host-native** tool call (`agent.memory.set`); the loop
//!     EXECUTES it through `call_tool` (previously `NotFound` via the registry-only path); the run
//!     settles with the tool's effect visible in the store. The exact path that was dead before.
//!   - **Capability-deny** — a run whose `agent_caps ∩ caller` lacks the tool's cap has the proposed
//!     host-native call `Denied` and fed back (not executed); a configured model grants no tool
//!     authority.
//!   - **Workspace-isolation** — a ws-B run cannot reach ws-A memory through the loop's dispatch.
//!   - **Unconfigured→configured swap** — `UnconfiguredModel` returns the unconfigured answer and
//!     proposes no tools; after `install_runtimes` with the real model the same invoke runs the loop.
//!   - **Tool menu = reachable catalog** — the loop's `AllowedTool` list equals the caller's reachable
//!     `tools.catalog` (a tool the caller can't run is absent AND denied if proposed).
//!   - **External-agent parity** — an external run's tool call reaches a host-native verb through the
//!     same `call_tool` wall (the shared dispatch serves both fronts).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, invoke, invoke_via_runtime, memory_get, reachable_tools, tools_catalog, AgentError,
    AgentRuntime, AllowedTool, ErasedModel, Invocation, Node, RunContext, RuntimeRegistry,
    Substrate, UnconfiguredModel, UNCONFIGURED_ANSWER,
};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider, ToolCall};
use std::future::Future;
use std::pin::Pin;

// ── caps ─────────────────────────────────────────────────────────────────────────────────────────
const INVOKE: &str = "mcp:agent.invoke:call";
const CATALOG: &str = "mcp:tools.catalog:call";
const MEM_SET: &str = "mcp:agent.memory.set:call";
const MEM_GET: &str = "mcp:agent.memory.get:call";
/// The distinct workspace-scope write gate: writing SHARED (`workspace`) memory needs this on TOP of
/// the verb cap. Using workspace scope makes the written row identity-independent (readable by the
/// caller regardless of the run's derived `agent:session` sub), so the effect is directly assertable.
const WS_WRITE: &str = "store:agent_memory/workspace:write";

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

/// A model that proposes ONE `agent.memory.set` (a host-native verb), then stops. `input` is the
/// verb's MCP arg shape `{scope, slug, description, kind, body}`.
fn set_memory_then_stop() -> AiGateway<MockProvider> {
    AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "I'll remember that.",
            vec![ToolCall {
                id: "c1".into(),
                name: "agent.memory.set".into(),
                input: r#"{"scope":"workspace","slug":"boiler-1-runs-hot","description":"boiler-1 runs hot","kind":"project","body":"Watch the temperature on boiler-1.","ts":1}"#.into(),
            }],
            10,
        ),
        AiResponse::stop("noted: boiler-1 runs hot", 5),
    ]))
}

/// The single allowed tool the model is told about — the host-native memory write.
fn mem_set_tool() -> Vec<AllowedTool> {
    vec![AllowedTool {
        name: "agent.memory.set".into(),
        description: "remember a fact".into(),
    }]
}

// ── the headline: the loop EXECUTES a host-native call through call_tool ──────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_in_house_loop_executes_a_host_native_tool_through_call_tool() {
    // THE dead path made live: a real AiGateway<MockProvider> as the in-house model → the loop runs →
    // the model proposes `agent.memory.set` (a HOST-NATIVE verb, unreachable through the old
    // registry-only `lb_mcp::call`) → the loop dispatches it through `call_tool` → the memory row lands.
    let ws = "ws-wiring-headline";
    let node = Arc::new(Node::boot().await.unwrap());
    // Caller (and agent ∩ caller) holds invoke + the memory write cap.
    let caller = principal("user:ada", ws, &[INVOKE, MEM_SET, MEM_GET, WS_WRITE]);
    let agent_caps: Vec<String> = vec![MEM_SET.into(), MEM_GET.into(), WS_WRITE.into()];

    let gw = set_memory_then_stop();
    let answer = invoke(
        &node,
        &gw,
        &caller,
        &agent_caps,
        ws,
        Invocation {
            job_id: "sess-headline",
            goal: "remember boiler-1 runs hot",
            skill: None,
            doc: None,
            tools: &mem_set_tool(),
            ts: 1,
        },
    )
    .await
    .expect("the in-house loop runs to completion");
    assert_eq!(answer, "noted: boiler-1 runs hot");

    // THE EFFECT IS IN THE STORE — the host-native write actually executed (was `NotFound` before).
    let mem = memory_get(
        &node.store,
        &caller,
        ws,
        Some("workspace"),
        "boiler-1-runs-hot",
    )
    .await
    .expect("memory read ok")
    .expect("the memory row the loop wrote is present");
    assert_eq!(mem.slug, "boiler-1-runs-hot");
    assert!(mem.body.contains("temperature"));

    // The transcript records the proposed host-native call AND an OK result (not an error).
    let job = lb_jobs::load(&node.store, ws, "sess-headline")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, lb_jobs::JobStatus::Done);
    assert!(job.events().any(|e| matches!(
        e,
        lb_jobs::TranscriptEvent::ToolCallProposed { name, .. } if name == "agent.memory.set"
    )));
    assert!(job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::ToolResult { ok: Some(_), .. })));
}

// ── capability-deny: a host-native call the intersection forbids is Denied, not executed ──────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_host_native_call_the_intersection_forbids_is_denied_and_fed_back() {
    // MANDATORY deny (testing §2.1): the AGENT lists the memory-set cap but the CALLER does NOT, so
    // `agent ∩ caller` lacks it — the proposed host-native call is DENIED inside the loop, fed back to
    // the model (not a crash), and NOTHING is persisted. A configured model grants no tool authority.
    let ws = "ws-wiring-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, MEM_GET]); // can invoke + read, NOT write memory
    let agent_caps: Vec<String> = vec![MEM_SET.into(), MEM_GET.into()]; // agent has it; caller doesn't

    let gw = set_memory_then_stop();
    let answer = invoke(
        &node,
        &gw,
        &caller,
        &agent_caps,
        ws,
        Invocation {
            job_id: "sess-deny",
            goal: "remember something",
            skill: None,
            doc: None,
            tools: &mem_set_tool(),
            ts: 1,
        },
    )
    .await
    .expect("the loop completes even though the tool was denied");
    assert_eq!(answer, "noted: boiler-1 runs hot");

    // The write NEVER landed — the wall held under the derived principal.
    let mem = memory_get(
        &node.store,
        &caller,
        ws,
        Some("workspace"),
        "boiler-1-runs-hot",
    )
    .await
    .expect("memory read ok");
    assert!(mem.is_none(), "the denied write must not have persisted");

    // The denial was fed back as a tool ERROR result (the model was told).
    let job = lb_jobs::load(&node.store, ws, "sess-deny")
        .await
        .unwrap()
        .unwrap();
    assert!(job
        .events()
        .any(|e| matches!(e, lb_jobs::TranscriptEvent::ToolResult { err: Some(_), .. })));
}

// ── workspace-isolation: a ws-B run cannot reach ws-A memory through the loop's dispatch ──────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_run_cannot_reach_ws_a_memory_through_the_loop() {
    // MANDATORY workspace-isolation (testing §2.2): ada in ws-A writes a memory via a real loop; a
    // separate run in ws-B (same slug) that tries to READ it gets nothing — `call_tool` is
    // workspace-first, so the loop's dispatch is walled per workspace.
    let node = Arc::new(Node::boot().await.unwrap());
    let ws_a = "ws-wiring-iso-a";
    let ws_b = "ws-wiring-iso-b";

    // ws-A: write the memory through the loop.
    let ada = principal("user:ada", ws_a, &[INVOKE, MEM_SET, MEM_GET, WS_WRITE]);
    invoke(
        &node,
        &set_memory_then_stop(),
        &ada,
        &[MEM_SET.into(), MEM_GET.into(), WS_WRITE.into()],
        ws_a,
        Invocation {
            job_id: "a-1",
            goal: "remember",
            skill: None,
            doc: None,
            tools: &mem_set_tool(),
            ts: 1,
        },
    )
    .await
    .unwrap();
    assert!(
        memory_get(
            &node.store,
            &ada,
            ws_a,
            Some("workspace"),
            "boiler-1-runs-hot"
        )
        .await
        .unwrap()
        .is_some(),
        "ws-A wrote its own memory"
    );

    // ws-B: a run whose model proposes a READ of the SAME slug — the read resolves in ws-B, which has
    // no such row. The workspace wall means the ws-A row is structurally unreachable.
    let bob = principal("user:bob", ws_b, &[INVOKE, MEM_GET]);
    let gw_read = AiGateway::new(MockProvider::new(vec![
        AiResponse::calls(
            "reading",
            vec![ToolCall {
                id: "r1".into(),
                name: "agent.memory.get".into(),
                input: r#"{"scope":"workspace","slug":"boiler-1-runs-hot"}"#.into(),
            }],
            5,
        ),
        AiResponse::stop("done", 1),
    ]));
    invoke(
        &node,
        &gw_read,
        &bob,
        &[MEM_GET.into()],
        ws_b,
        Invocation {
            job_id: "b-1",
            goal: "read",
            skill: None,
            doc: None,
            tools: &[AllowedTool {
                name: "agent.memory.get".into(),
                description: "read".into(),
            }],
            ts: 1,
        },
    )
    .await
    .unwrap();

    // Direct proof at the store: ws-B holds nothing of ws-A's memory.
    assert!(
        memory_get(
            &node.store,
            &bob,
            ws_b,
            Some("workspace"),
            "boiler-1-runs-hot"
        )
        .await
        .unwrap()
        .is_none(),
        "ws-B cannot see ws-A's memory row"
    );
}

// ── unconfigured → configured swap: the seam is the registry, not a code branch ───────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unconfigured_returns_the_honest_answer_then_configured_runs_the_loop() {
    // The swap: with `UnconfiguredModel` the in-house default returns UNCONFIGURED_ANSWER and proposes
    // no tools; after `install_runtimes` with a real model the SAME invoke runs the loop and writes the
    // memory. Nothing else changes — the difference is config (the installed registry), never a branch.
    let ws = "ws-wiring-swap";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, MEM_SET, MEM_GET, WS_WRITE]);

    // Boot posture: the default registry binds `UnconfiguredModel`.
    node.install_runtimes(RuntimeRegistry::with_default(Arc::new(UnconfiguredModel)));
    let answer = run_default(&node, &caller, ws, "swap-0").await;
    assert_eq!(
        answer, UNCONFIGURED_ANSWER,
        "unconfigured gives the honest answer"
    );
    assert!(
        memory_get(
            &node.store,
            &caller,
            ws,
            Some("workspace"),
            "boiler-1-runs-hot"
        )
        .await
        .unwrap()
        .is_none(),
        "unconfigured proposes no tools — nothing written"
    );

    // Configure: install the in-house default over a real AiGateway<MockProvider>.
    let model: Arc<dyn ErasedModel> = Arc::new(set_memory_then_stop());
    node.install_runtimes(RuntimeRegistry::with_default(model));
    let answer = run_default(&node, &caller, ws, "swap-1").await;
    assert_eq!(
        answer, "noted: boiler-1 runs hot",
        "configured runs the loop"
    );
    assert!(
        memory_get(
            &node.store,
            &caller,
            ws,
            Some("workspace"),
            "boiler-1-runs-hot"
        )
        .await
        .unwrap()
        .is_some(),
        "configured executed the host-native write"
    );
}

/// Drive a `runtime:"default"` run against the NODE's installed registry (what the production
/// entrypoints do), surfacing the caller's reachable tools — exactly the channel-worker path.
async fn run_default(node: &Arc<Node>, caller: &Principal, ws: &str, job: &str) -> String {
    let registry = node.runtimes();
    let tools = reachable_tools(node, caller, ws).await;
    invoke_via_runtime(
        node,
        &registry,
        None, // absent runtime → the in-house default
        caller,
        &caller.caps().to_vec(),
        ws,
        job,
        "remember boiler-1 runs hot",
        Substrate::default(),
        &tools,
        1,
    )
    .await
    .expect("run completes")
}

// ── tool menu = reachable catalog: absent from the menu AND denied if proposed ────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_loop_menu_equals_the_callers_reachable_catalog() {
    // The menu the loop is given EQUALS `tools.catalog` for the caller (one gate, two callers) — the
    // menu is the reachable catalog, so it can never advertise a tool that would then deny, nor hide
    // one that would pass. A tool the caller cannot run is ABSENT from BOTH.
    //
    // Discriminator: `agent.invoke` is a catalog descriptor gated by `mcp:agent.invoke:call`. A caller
    // holding it sees `agent.invoke` in both menu and catalog; a caller without it sees neither — even
    // though both hold `mcp:tools.catalog:call` (so the catalog itself is readable).
    let ws = "ws-wiring-menu";
    let node = Arc::new(Node::boot().await.unwrap());

    // (a) With the invoke cap: the menu equals the catalog AND lists `agent.invoke`.
    let with_invoke = principal("user:ada", ws, &[CATALOG, INVOKE]);
    let menu = reachable_tools(&node, &with_invoke, ws).await;
    let catalog = tools_catalog(&node, &with_invoke, ws)
        .await
        .expect("catalog ok");
    let menu_names: Vec<String> = menu.iter().map(|t| t.name.clone()).collect();
    let catalog_names: Vec<String> = catalog.tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(
        menu_names, catalog_names,
        "the loop's AllowedTool menu equals the reachable tools.catalog"
    );
    assert!(
        menu_names.iter().any(|n| n == "agent.invoke"),
        "a runnable catalog tool appears in the menu"
    );

    // (b) WITHOUT the invoke cap: the same equality holds, and `agent.invoke` is absent from the menu —
    // the menu is exactly what the caller may run, no more.
    let no_invoke = principal("user:bob", ws, &[CATALOG]);
    let menu2 = reachable_tools(&node, &no_invoke, ws).await;
    let catalog2 = tools_catalog(&node, &no_invoke, ws)
        .await
        .expect("catalog ok");
    let menu2_names: Vec<String> = menu2.iter().map(|t| t.name.clone()).collect();
    let catalog2_names: Vec<String> = catalog2.tools.iter().map(|t| t.name.clone()).collect();
    assert_eq!(
        menu2_names, catalog2_names,
        "menu equals catalog for this caller too"
    );
    assert!(
        !menu2_names.iter().any(|n| n == "agent.invoke"),
        "a tool the caller lacks the cap for is ABSENT from the menu"
    );
}

// ── external-agent parity: an external run reaches a host-native verb through the same wall ────────

/// A minimal external-style runtime: it does NOT use the in-house loop — it dispatches a host-native
/// tool call itself through the SHARED `call_tool` bridge under the derived principal (`agent ∩
/// caller`), exactly as the external ACP runtime's tool bridge does. This proves the shared dispatch
/// serves the external front too: the same wall, the same host-native reach.
struct ExternalStub {
    id: String,
    call: String,
    input: String,
}

impl AgentRuntime for ExternalStub {
    fn id(&self) -> &str {
        &self.id
    }
    fn run<'a>(
        &'a self,
        node: &'a Arc<Node>,
        ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        Box::pin(async move {
            // Derive `agent ∩ caller` exactly as the in-house loop does, then reach a host-native verb
            // through the ONE bridge — the shared dispatch the default-agent-wiring fix installs.
            let agent = ctx.caller.derive("agent:external", ctx.agent_caps.to_vec());
            match call_tool(node, &agent, ctx.ws, &self.call, &self.input).await {
                Ok(_) => Ok("external reached the host verb".into()),
                Err(_) => Ok("external denied".into()),
            }
        })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_external_run_reaches_a_host_native_verb_through_the_same_wall() {
    let ws = "ws-wiring-external";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, MEM_SET, MEM_GET, WS_WRITE]);

    // Register an external runtime that writes memory via `call_tool`, then drive it.
    let mut registry = RuntimeRegistry::with_default(Arc::new(UnconfiguredModel));
    registry.register(Arc::new(ExternalStub {
        id: "ext-writer".into(),
        call: "agent.memory.set".into(),
        input: r#"{"scope":"workspace","slug":"ext-fact","description":"from external","kind":"project","body":"external wrote this.","ts":1}"#.into(),
    }));
    node.install_runtimes(registry);

    let answer = invoke_via_runtime(
        &node,
        &node.runtimes(),
        Some("ext-writer"),
        &caller,
        &[MEM_SET.into(), MEM_GET.into(), WS_WRITE.into()],
        ws,
        "ext-1",
        "write via external",
        Substrate::default(),
        &[],
        1,
    )
    .await
    .expect("external run completes");
    assert_eq!(answer, "external reached the host verb");

    // The external run's host-native write landed through the SAME wall the in-house loop uses.
    assert!(
        memory_get(&node.store, &caller, ws, Some("workspace"), "ext-fact")
            .await
            .unwrap()
            .is_some(),
        "the external run reached a host-native verb through call_tool"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_external_run_host_native_call_is_denied_when_the_intersection_forbids_it() {
    // Parity deny: the external front is walled identically — a caller lacking the write cap denies the
    // external run's host-native write, nothing persists (model access / runtime choice grants nothing).
    let ws = "ws-wiring-external-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, MEM_GET]); // no write cap

    let mut registry = RuntimeRegistry::with_default(Arc::new(UnconfiguredModel));
    registry.register(Arc::new(ExternalStub {
        id: "ext-writer".into(),
        call: "agent.memory.set".into(),
        input: r#"{"scope":"workspace","slug":"ext-fact","description":"x","kind":"project","body":"y","ts":1}"#.into(),
    }));
    node.install_runtimes(registry);

    let answer = invoke_via_runtime(
        &node,
        &node.runtimes(),
        Some("ext-writer"),
        &caller,
        &[MEM_SET.into(), MEM_GET.into()], // agent has it; caller does not → intersection lacks it
        ws,
        "ext-deny-1",
        "write via external",
        Substrate::default(),
        &[],
        1,
    )
    .await
    .expect("external run completes");
    assert_eq!(answer, "external denied");
    assert!(
        memory_get(&node.store, &caller, ws, Some("workspace"), "ext-fact")
            .await
            .unwrap()
            .is_none(),
        "the denied external write must not persist"
    );
}

// ── offline/sync: a mid-run disconnect resumes and re-drives a host-native call cleanly ────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_resumed_run_redrives_a_host_native_call_through_the_new_dispatch() {
    // MANDATORY offline/sync (testing §2.3) + the scope's "the new dispatch must not break rehydrate":
    // a durable session that ran turn 0 (an assistant turn) then disconnected (status Running) is
    // RESUMED; the resume turn proposes a HOST-NATIVE `agent.memory.set` — re-driven through the new
    // `call_tool` dispatch — and the run settles with the effect in the store. Proves rehydrate +
    // the new dispatch compose.
    use lb_jobs::{append_event, create, Job, JobStatus, TranscriptEvent};

    let ws = "ws-wiring-resume";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal("user:ada", ws, &[INVOKE, MEM_SET, MEM_GET, WS_WRITE]);

    // Seed a durable partial state: turn 0's assistant text persisted, loop never finished.
    let job = Job::new(
        "sess-resume",
        "agent-session",
        "remember boiler-1 runs hot",
        1,
    );
    create(&node.store, ws, &job).await.unwrap();
    append_event(
        &node.store,
        ws,
        "sess-resume",
        0,
        TranscriptEvent::AssistantTurn {
            content: "working on it".into(),
        },
    )
    .await
    .unwrap();
    let seeded = lb_jobs::load(&node.store, ws, "sess-resume")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        seeded.status,
        JobStatus::Running,
        "the session did not finish"
    );

    // RESUME: the model's script begins at the resume turn — propose the host-native write, then stop.
    let gw = set_memory_then_stop();
    let answer = lb_host::resume(
        &node,
        &gw,
        &caller,
        &[MEM_SET.into(), MEM_GET.into(), WS_WRITE.into()],
        ws,
        "sess-resume",
        &mem_set_tool(),
        1,
    )
    .await
    .expect("resume continues the session");
    assert_eq!(answer, "noted: boiler-1 runs hot");

    let done = lb_jobs::load(&node.store, ws, "sess-resume")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(done.status, JobStatus::Done, "resumed to completion");
    // The resumed host-native write landed through the new dispatch.
    assert!(
        memory_get(
            &node.store,
            &caller,
            ws,
            Some("workspace"),
            "boiler-1-runs-hot"
        )
        .await
        .unwrap()
        .is_some(),
        "the resumed run re-drove the host-native call through call_tool"
    );
    // The pre-disconnect assistant turn survived untouched (not re-run, not duplicated).
    assert!(matches!(
        &done.steps[0].event,
        TranscriptEvent::AssistantTurn { content } if content == "working on it"
    ));
}
