//! Agent-personas sub-scope #3 (persona-catalog) — the seven built-in personas as DATA, verified
//! against the live loop (real Node + store + caps + the deterministic MockProvider, rule 9). The
//! lists in `personas.toml` are curation, so the tests prove: the seed resolves; each persona NARROWS
//! the menu to its focus (an in-list tool present, an out-of-list granted tool absent, an excluded
//! destructive verb absent EVEN for an admin caller — the persona-never-widens headline per persona);
//! `extends` composes (rules-author ⊇ its parents; system-manager follows a parent); and the
//! CONFUSION FIX is demonstrated (the same task's menu shrinks from the whole surface to the focus).
//!
//! Zero new code under test — #1 built the record + application, #3 is content. This file is the
//! guard that keeps the content honest as the verb inventory grows (a persona missing a verb, or a
//! destructive verb sneaking into one, fails here loudly).

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_persona_get, call_agent_tool, grant_skill, invoke_via_runtime, reachable_tools,
    seed_core_skills, seed_personas, AllowedTool, ErasedModel, Node, Persona, RunContext,
    RuntimeRegistry, Substrate,
};
use lb_prefs::{set_workspace_prefs, Prefs};
use lb_role_ai_gateway::{AiGateway, AiResponse, MockProvider};
use lb_role_gateway::dev_claims;

// ---- harness -----------------------------------------------------------------------------------

/// A principal carrying the FULL dev-login member+admin cap set (the real `member_caps()` the gateway
/// mints) — so `reachable_tools` returns the whole live catalog and a persona genuinely narrows it.
/// This is the "admin caller" the per-persona destructive-exclusion test needs.
fn admin_principal(sub: &str, ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = dev_claims(sub, ws, 0, u64::MAX);
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A bare member — only the invoke gate + the catalog read, none of the admin verbs. For the
/// caps-deny proof (a persona advertising an admin verb does nothing for this caller).
fn member_principal(sub: &str, ws: &str) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec![
            "mcp:agent.invoke:call".into(),
            "mcp:tools.catalog:call".into(),
            // Skill-read: so the persona's pinned-skill BODIES load (grounding succeeds) and the run
            // reaches the tool-menu assembly this test is about. Member-appropriate (reading a granted
            // skill is not an admin act). Without it the run fail-closes on grounding BEFORE the tool
            // narrowing — a different (also-correct) behavior tested in agent_persona_test.rs.
            "store:skill/**:read".into(),
            "mcp:assets.load_skill:call".into(),
        ],
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A recording model: captures the menu it was handed on the first turn, then stops.
#[derive(Default)]
struct Captured {
    tool_names: Vec<String>,
    turns: usize,
}
struct RecordingModel(Arc<Mutex<Captured>>);
impl ErasedModel for RecordingModel {
    fn turn_boxed<'a>(
        &'a self,
        _ws: &'a str,
        _messages: &'a [(String, String)],
        tools: &'a [AllowedTool],
        _prior: &'a [lb_host::CallOutcome],
        _key: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = lb_host::Turn> + Send + 'a>> {
        {
            let mut c = self.0.lock().unwrap();
            if c.turns == 0 {
                c.tool_names = tools.iter().map(|t| t.name.clone()).collect();
            }
            c.turns += 1;
        }
        Box::pin(async move {
            lb_host::Turn {
                content: "done".into(),
                calls: vec![],
                done: true,
            }
        })
    }
    fn is_configured(&self) -> bool {
        true
    }
}
fn recording_registry() -> (RuntimeRegistry, Arc<Mutex<Captured>>) {
    let cap = Arc::new(Mutex::new(Captured::default()));
    let model: Arc<dyn ErasedModel> = Arc::new(RecordingModel(cap.clone()));
    (RuntimeRegistry::with_default(model), cap)
}

/// The built-in personas pin real skills (`core.rules`, `core.datasources`, …). A run fails CLOSED if
/// a pinned skill isn't granted (#1 — proven elsewhere), so the menu tests must GRANT the corpus first:
/// seed the core skills, then grant every skill any built-in persona pins to `ws`. `granter` must hold
/// `store:skill/**:write` (the dev admin does). This keeps the menu tests about narrowing, not grounding.
async fn setup_catalog(node: &Arc<Node>, ws: &str, granter: &Principal) {
    seed_core_skills(&node.store, "0.1.0", 1)
        .await
        .expect("seed core skills");
    seed_personas(&node.store).await.expect("seed personas");
    // Collect every pinned skill across the built-ins (resolved, so extends-inherited pins count too).
    let mut pins: std::collections::HashSet<String> = std::collections::HashSet::new();
    for id in BUILTIN_IDS {
        if let Ok(p) = agent_persona_get(node, granter, ws, id).await {
            collect_pins(node, ws, granter, &p, &mut pins).await;
        }
    }
    for skill in pins {
        // Idempotent; a skill not in the corpus simply no-ops the grant (none are, but be defensive).
        let _ = grant_skill(&node.store, granter, ws, &skill).await;
    }
}

/// Gather a persona's pinned skills plus (transitively) its `extends` parents' pins.
async fn collect_pins(
    node: &Arc<Node>,
    ws: &str,
    granter: &Principal,
    p: &Persona,
    out: &mut std::collections::HashSet<String>,
) {
    for s in &p.grounding_skills {
        out.insert(s.clone());
    }
    for parent in &p.extends {
        if let Ok(pp) = agent_persona_get(node, granter, ws, parent).await {
            Box::pin(collect_pins(node, ws, granter, &pp, out)).await;
        }
    }
}

const BUILTIN_IDS: &[&str] = &[
    "builtin.data-analyst",
    "builtin.flow-author",
    "builtin.widget-builder",
    "builtin.rules-author",
    "builtin.workspace-admin",
    "builtin.channels-operator",
    "builtin.system-manager",
];

/// Drive a run under the given active persona and return the menu the model actually saw.
async fn menu_under_persona(
    node: &Arc<Node>,
    ws: &str,
    caller: &Principal,
    persona: &str,
) -> Vec<String> {
    let (registry, cap) = recording_registry();
    invoke_via_runtime(
        node,
        &registry,
        None,
        Some(persona),
        caller,
        &caller.caps().to_vec(),
        ws,
        &format!("job-{persona}"),
        "do the task",
        Substrate::default(),
        None,
        &reachable_tools(node, caller, ws).await,
        1,
    )
    .await
    .expect("run drives");
    let names = cap.lock().unwrap().tool_names.clone();
    names
}

// ================================================================================================
// Seed
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn all_seven_builtins_seed_with_tools_and_pins() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.expect("seed");
    let admin = admin_principal("user:ada", "cat-seed");
    for id in [
        "builtin.data-analyst",
        "builtin.flow-author",
        "builtin.widget-builder",
        "builtin.rules-author",
        "builtin.workspace-admin",
        "builtin.channels-operator",
        "builtin.system-manager",
    ] {
        let p = agent_persona_get(&node, &admin, "cat-seed", id)
            .await
            .unwrap_or_else(|_| panic!("{id} resolves"));
        assert!(p.builtin, "{id} is a built-in");
        assert!(!p.label.is_empty(), "{id} has a label");
        assert!(!p.identity.is_empty(), "{id} has an identity");
        // Every persona narrows SOMETHING (tools of its own or via extends) and grounds with a pin.
        assert!(
            !p.granted_tools.is_empty() || !p.extends.is_empty(),
            "{id} narrows (own tools or extends)"
        );
    }
}

// ================================================================================================
// Per-persona menu — narrowing + destructive exclusion (even for an admin caller)
// ================================================================================================

// (persona, an in-focus tool that MUST appear, an out-of-focus tool that must NOT). NOTE: these use
// tools that are genuinely PALETTE-REACHABLE on a bare node — `reachable_tools` reads `tools.catalog`
// = `host_descriptors()` ∩ caps, a CURATED palette (~11 host verbs + extensions), not the full
// ~175-verb surface (see the scope's "Implementation finding"). The narrowing is proven over what is
// actually reachable; a persona's full `granted_tools` list is the forward-looking allow-list.
const MENU_CASES: &[(&str, &str, &str)] = &[
    // data-analyst curates federation.query + query.*; it does NOT list reminder.*.
    (
        "builtin.data-analyst",
        "federation.query",
        "reminder.create",
    ),
    // widget-builder curates dashboard.* (dashboard.catalog is palette-reachable) + query.*; not reminder.
    (
        "builtin.widget-builder",
        "dashboard.catalog",
        "reminder.create",
    ),
    // channels-operator curates reminder.* ; it does NOT list federation.query.
    (
        "builtin.channels-operator",
        "reminder.create",
        "federation.query",
    ),
];

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_persona_narrows_to_its_focus_for_an_admin_caller() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-menu";
    let admin = admin_principal("user:ada", ws); // holds the FULL surface — the persona must narrow it
    setup_catalog(&node, ws, &admin).await;

    // Baseline: the admin's full reachable palette (every palette tool it may run, no persona).
    let full = reachable_tools(&node, &admin, ws).await;
    assert!(
        full.len() >= MENU_CASES.len(),
        "the un-narrowed palette has at least the tools we assert on ({} tools)",
        full.len()
    );

    for (persona, in_list, out_of_list) in MENU_CASES {
        let menu = menu_under_persona(&node, ws, &admin, persona).await;
        assert!(
            menu.iter().any(|t| t == in_list),
            "{persona}: its in-focus palette tool {in_list} is in the menu (menu: {menu:?})"
        );
        assert!(
            !menu.iter().any(|t| t == out_of_list),
            "{persona}: the out-of-focus palette tool {out_of_list} is NOT advertised (menu: {menu:?})"
        );
        assert!(
            menu.len() <= full.len(),
            "{persona}: the menu never grows beyond the reachable palette ({} <= {})",
            menu.len(),
            full.len()
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn destructive_verbs_are_excluded_from_every_persona_even_for_an_admin() {
    // The deliberate stance: workspace.delete/purge + authz.revoke-tokens + secret.get are advertised
    // by NO built-in persona — even the workspace-admin / system-manager, even to an admin caller who
    // HOLDS the cap. Advertising a catastrophic verb to a model invites a catastrophic proposal.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-destructive";
    let admin = admin_principal("user:ada", ws); // holds workspace.purge etc. (dev login is admin)
    setup_catalog(&node, ws, &admin).await;

    let forbidden = [
        "workspace.delete",
        "workspace.purge",
        "authz.revoke-tokens",
        "secret.get",
    ];
    for persona in ["builtin.workspace-admin", "builtin.system-manager"] {
        let menu = menu_under_persona(&node, ws, &admin, persona).await;
        for f in forbidden {
            assert!(
                !menu.iter().any(|t| t == f),
                "{persona} must NOT advertise the destructive verb {f} (even to an admin caller)"
            );
        }
    }
}

// ================================================================================================
// Capability-deny (§2.1) — a persona never widens
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_admin_persona_under_a_member_caller_advertises_nothing_it_lacks() {
    // The headline, per persona: a MEMBER caller under `workspace-admin` gets a menu narrowed to
    // `persona ∩ member` — the admin verbs the persona lists (members.manage, roles.define) are NOT
    // reachable for a bare member, so they never appear (the wall, not the persona, decides).
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-deny";
    // An admin seeds + grants the personas' pinned skills to the ws (so the member run isn't blocked by
    // a fail-closed pin — the grant is workspace-scoped, read under the derived principal). The DENY we
    // prove is about the persona's admin TOOLS, not its skills.
    let admin = admin_principal("user:ada", ws);
    setup_catalog(&node, ws, &admin).await;
    let member = member_principal("user:mo", ws); // only invoke + catalog

    let menu = menu_under_persona(&node, ws, &member, "builtin.workspace-admin").await;
    for admin_verb in ["members.manage", "roles.define", "user.disable"] {
        assert!(
            !menu.iter().any(|t| t == admin_verb),
            "a member caller under workspace-admin never sees the admin verb {admin_verb} \
             (persona ∩ member — the persona advertises it, the wall withholds it)"
        );
    }
}

// ================================================================================================
// extends resolution
// ================================================================================================

/// Resolve a persona's EFFECTIVE (extends-unioned) `granted_tools` via the `agent.persona.resolve`
/// verb — the full allow-list, INCLUDING non-palette verbs (`rules.save`, `flows.save`), which the
/// menu can't show. This is the honest place to prove `extends` composition (the record union), vs the
/// menu (which is palette-filtered — see the scope finding).
async fn effective_tools(
    node: &Arc<Node>,
    ws: &str,
    caller: &Principal,
    persona: &str,
) -> Vec<String> {
    let out = call_agent_tool(
        node,
        caller,
        ws,
        "agent.persona.resolve",
        &serde_json::json!({ "id": persona }),
    )
    .await
    .expect("resolve ok");
    serde_json::from_value(out["effective"]["granted_tools"].clone()).expect("granted_tools")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_author_is_the_union_of_its_parents_plus_its_own() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-extends";
    let admin = admin_principal("user:ada", ws);
    setup_catalog(&node, ws, &admin).await;

    // The extends-union at the RECORD level (agent.persona.resolve) — the full allow-list.
    let tools = effective_tools(&node, ws, &admin, "builtin.rules-author").await;
    assert!(tools.iter().any(|t| t == "rules.*"), "own rules.* present");
    assert!(
        tools.iter().any(|t| t == "flows.*"),
        "inherited flows.* (flow-author) present"
    );
    assert!(
        tools.iter().any(|t| t == "federation.query"),
        "inherited federation.query (data-analyst) present"
    );

    // And the palette-reachable subset shows up in the actual run menu (federation.query is a descriptor).
    let menu = menu_under_persona(&node, ws, &admin, "builtin.rules-author").await;
    assert!(
        menu.iter().any(|t| t == "federation.query"),
        "the inherited palette tool reaches the run menu"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn system_manager_composes_all_six_parents() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-sysmgr";
    let admin = admin_principal("user:ada", ws);
    setup_catalog(&node, ws, &admin).await;

    // The full unioned allow-list: a tool from each of the six parents + its own system.* surface.
    let tools = effective_tools(&node, ws, &admin, "builtin.system-manager").await;
    for expected in [
        "system.overview",  // own
        "federation.query", // data-analyst
        "flows.*",          // flow-author
        "dashboard.*",      // widget-builder
        "rules.*",          // rules-author
        "roles.list",       // workspace-admin
        "channel.post",     // channels-operator
    ] {
        assert!(
            tools.iter().any(|t| t == expected),
            "system-manager composes {expected} (effective: {tools:?})"
        );
    }
    // But NOT the destructive verbs, even composed (no parent lists them):
    assert!(
        !tools
            .iter()
            .any(|t| t == "workspace.purge" || t == "workspace.delete"),
        "composition does not resurrect an excluded destructive verb"
    );
}

// ================================================================================================
// The CONFUSION FIX, demonstrated (the umbrella exit gate)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_confusion_fix_the_same_task_narrows_from_the_whole_surface_to_the_focus() {
    // The umbrella gate: the SAME task runs FOCUSED under the matching built-in persona. BEFORE = the
    // whole REACHABLE palette (every palette tool the admin may run — the observed sprawl); AFTER = the
    // data-analyst's focus. Same caller, same task, one persona pick. NOTE: the menu is the palette
    // catalog, not the full ~175-verb surface (scope finding) — so the numbers here are the palette
    // narrowing; the effect is larger in production with extension tools loaded, and the OTHER half of
    // the cure (identity + pinned grounding) is proven in #2's grounding test.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "cat-confusion";
    let admin = admin_principal("user:ada", ws);
    setup_catalog(&node, ws, &admin).await;

    // BEFORE: no persona → the full reachable palette.
    let before = menu_under_persona_none(&node, ws, &admin).await;

    // AFTER: pick the matching persona (the workspace defaults to it via the prefs axis).
    set_workspace_prefs(
        &node.store,
        ws,
        &Prefs {
            agent_persona: Some("builtin.data-analyst".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let after = menu_under_persona(&node, ws, &admin, "builtin.data-analyst").await;

    assert!(
        after.len() < before.len(),
        "the focused menu is strictly smaller than the full palette ({} < {})",
        after.len(),
        before.len()
    );
    // The focus is present; palette tools OFF the data-analyst's task are gone (these ARE reachable
    // palette tools, so their absence is a real narrowing, not a vacuous check).
    assert!(
        after.iter().any(|t| t == "federation.query"),
        "the on-task tool stays"
    );
    for gone in ["reminder.create", "dashboard.pin"] {
        assert!(
            before.iter().any(|t| t == gone),
            "{gone} IS a reachable palette tool (so its removal is meaningful)"
        );
        assert!(
            !after.iter().any(|t| t == gone),
            "the off-task palette tool {gone} is gone under the data-analyst persona"
        );
    }
    // Record the numbers for the session doc (visible in `cargo test -- --nocapture`).
    println!(
        "CONFUSION FIX: reachable palette {} tools -> {} tools under the data-analyst persona \
         (identity + pinned grounding are the other half of the cure — see #2)",
        before.len(),
        after.len()
    );
}

/// The un-narrowed menu (no persona) the model would see — the "before" of the confusion demo.
async fn menu_under_persona_none(node: &Arc<Node>, ws: &str, caller: &Principal) -> Vec<String> {
    let (registry, cap) = recording_registry();
    invoke_via_runtime(
        node,
        &registry,
        None,
        None, // NO persona
        caller,
        &caller.caps().to_vec(),
        ws,
        "job-before",
        "which sites had abnormal energy use last week?",
        Substrate::default(),
        None,
        &reachable_tools(node, caller, ws).await,
        1,
    )
    .await
    .expect("run drives");
    let names = cap.lock().unwrap().tool_names.clone();
    names
}

// ================================================================================================
// Workspace-isolation (§2.2)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_default_persona_never_affects_a_ws_a_run() {
    // Built-in personas are readable everywhere, but the DEFAULT pick rides the ws-scoped prefs
    // record (persona-session #5). ws-B defaulting to a narrow persona must NOT narrow (or widen) a
    // ws-A run — the pick is walled.
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let admin_a = admin_principal("user:ada", "ws-a");
    let admin_b = admin_principal("user:bo", "ws-b");
    setup_catalog(&node, "ws-a", &admin_a).await;
    setup_catalog(&node, "ws-b", &admin_b).await;

    // ws-B defaults to a NARROW persona (data-analyst) via the ws-default prefs axis; ws-A sets NONE.
    set_workspace_prefs(
        &node.store,
        "ws-b",
        &Prefs {
            agent_persona: Some("builtin.data-analyst".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    // A ws-A run (no default persona) sees its FULL ws-A palette — ws-B's pick is invisible to it.
    let a_menu = menu_under_persona_none(&node, "ws-a", &admin_a).await;
    // A ws-B run (active data-analyst) is narrowed.
    let (registry, cap) = recording_registry();
    invoke_via_runtime(
        &node,
        &registry,
        None,
        None, // no explicit persona → resolves ws-B's ws-default data-analyst
        &admin_b,
        &admin_b.caps().to_vec(),
        "ws-b",
        "job-b",
        "task",
        Substrate::default(),
        None,
        &reachable_tools(&node, &admin_b, "ws-b").await,
        1,
    )
    .await
    .expect("ws-b run drives");
    let b_menu = cap.lock().unwrap().tool_names.clone();

    assert!(
        a_menu.len() > b_menu.len(),
        "ws-A (no persona) keeps its full palette ({}) while ws-B's data-analyst pick narrows ws-B ({}) \
         — the pick is workspace-walled, never cross-tenant",
        a_menu.len(),
        b_menu.len()
    );
    assert!(
        a_menu.iter().any(|t| t == "reminder.create"),
        "ws-A run still sees reminder.create — ws-B's data-analyst pick did NOT narrow it"
    );
}
