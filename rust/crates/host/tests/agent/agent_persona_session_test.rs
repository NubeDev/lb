//! Agent-personas sub-scope #5 (persona-session) — roster, prefs-axis defaults, and the retired
//! `active_persona` toggle, against a **real** Node + store + caps (rule 9; no mocks anywhere in
//! this file — every layer is real). The client half (context match + per-tab pin) lives in the UI
//! gateway tests; this file locks the server half:
//!
//!   - **Precedence table:** explicit invoke id > member default > ws default > none; a dangling
//!     member/ws default → `warn!` + un-narrowed; an explicit-but-unknown id → named `NotFound`;
//!     an explicit-but-**disabled** id → named `BadInput` (curation is not silently bypassable).
//!   - **Roster semantics:** `None` ⇒ all enabled (`agent.persona.list` flags every persona
//!     `enabled: true`); `Some(list)` flags the rest disabled + fails explicit disabled invokes; a
//!     roster naming a deleted persona is inert; an empty roster means "cleared → all enabled" (the
//!     MERGE-can't-write-null workaround).
//!   - **Workspace isolation (mandatory):** a ws-A member/ws default never affects a ws-B run; a
//!     ws-A custom id stored as ws-B's default folds to none + `warn!` (the namespace wall).
//!   - **Capability deny (mandatory):** member denied on `agent.config.set { enabled_personas }`
//!     and on `prefs.set_default { agent_persona }`; member allowed on `prefs.set { agent_persona }`.
//!   - **Independence (the headline):** two members of one workspace resolve different personas in
//!     the same tick with zero cross-writes; two explicit ids under one member both narrow — proof
//!     the per-tab pin needs no server state.
//!   - **Migration:** a legacy `agent.config.active_persona` is copied once into the ws-default
//!     prefs axis at boot, idempotently; an admin-set axis is never overwritten; unset legacy ⇒ no
//!     ws default appears.
//!   - **Surfaces as data (rule-10 swap half):** a custom persona with a novel `surfaces` entry
//!     round-trips through create → list — record edit only, zero code change (the dock-match half
//!     is `AgentDock.gateway.test.tsx`).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_config_set, agent_persona_create, agent_persona_delete, agent_persona_list,
    migrate_active_persona, prefs_set, prefs_set_default, register_workspace, resolve_persona,
    seed_personas, AgentConfig, Node, Persona, PrefsSvcError,
};
use lb_mcp::ToolError;
use lb_prefs::{get_workspace_prefs, set_user_prefs, set_workspace_prefs, Prefs};
use serde_json::Value;

// ---- harness -----------------------------------------------------------------------------------

const P_LIST: &str = "mcp:agent.persona.list:call";
const P_CREATE: &str = "mcp:agent.persona.create:call";
const P_DELETE: &str = "mcp:agent.persona.delete:call";
const CFG_SET: &str = "mcp:agent.config.set:call";
const PREFS_SET: &str = "mcp:prefs.set:call";
const PREFS_SET_DEFAULT: &str = "mcp:prefs.set_default:call";

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A minimal custom persona narrowing to one named tool.
fn persona(id: &str, tool: &str) -> Persona {
    Persona {
        id: id.into(),
        label: format!("Test {id}"),
        description: None,
        identity: format!("You are {id}."),
        granted_tools: vec![tool.into()],
        grounding_skills: vec![],
        extends: vec![],
        surfaces: vec![],
        policy_preset: None,
        runtimes: None,
        builtin: false,
    }
}

/// Write a workspace roster as an admin (the real gated verb).
async fn set_roster(node: &Arc<Node>, admin: &Principal, ws: &str, roster: Option<Vec<&str>>) {
    agent_config_set(
        node,
        admin,
        ws,
        &AgentConfig {
            enabled_personas: roster.map(|v| v.iter().map(|s| s.to_string()).collect()),
            ..Default::default()
        },
    )
    .await
    .expect("roster write");
}

// ================================================================================================
// Precedence table (host)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn precedence_explicit_beats_member_default_beats_ws_default_beats_none() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "sess-precedence";
    let caller = principal("user:ada", ws, &[P_CREATE]);
    for id in ["explicit-p", "member-p", "ws-p"] {
        agent_persona_create(&node, &caller, ws, &persona(id, "agent.memory.list"))
            .await
            .expect("create");
    }

    // Layer 4: nothing set → none.
    assert!(resolve_persona(&node, &caller, ws, None)
        .await
        .expect("resolves")
        .is_none());

    // Layer 3: ws default.
    set_workspace_prefs(
        &node.store,
        ws,
        &Prefs {
            agent_persona: Some("ws-p".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resolve_persona(&node, &caller, ws, None)
            .await
            .expect("resolves")
            .expect("ws default applies")
            .id,
        "ws-p"
    );

    // Layer 2: member default beats the ws default.
    set_user_prefs(
        &node.store,
        ws,
        "user:ada",
        &Prefs {
            agent_persona: Some("member-p".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resolve_persona(&node, &caller, ws, None)
            .await
            .expect("resolves")
            .expect("member default applies")
            .id,
        "member-p"
    );

    // Layer 1: explicit invoke id beats both.
    assert_eq!(
        resolve_persona(&node, &caller, ws, Some("explicit-p"))
            .await
            .expect("resolves")
            .expect("explicit applies")
            .id,
        "explicit-p"
    );

    // An empty-string member default is "cleared" — falls to the ws default.
    set_user_prefs(
        &node.store,
        ws,
        "user:ada",
        &Prefs {
            agent_persona: Some(String::new()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert_eq!(
        resolve_persona(&node, &caller, ws, None)
            .await
            .expect("resolves")
            .expect("cleared member axis falls through")
            .id,
        "ws-p"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dangling_default_warns_and_runs_un_narrowed_an_explicit_unknown_errors() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "sess-dangling";
    let caller = principal("user:ada", ws, &[P_CREATE, P_DELETE]);

    // Dangling ws default (persona never existed) → None, not an error.
    set_workspace_prefs(
        &node.store,
        ws,
        &Prefs {
            agent_persona: Some("ghost".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(resolve_persona(&node, &caller, ws, None)
        .await
        .expect("no error for a dangling default")
        .is_none());

    // Dangling member default (created then deleted) → same posture.
    agent_persona_create(&node, &caller, ws, &persona("temp", "agent.memory.list"))
        .await
        .unwrap();
    set_user_prefs(
        &node.store,
        ws,
        "user:ada",
        &Prefs {
            agent_persona: Some("temp".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    agent_persona_delete(&node, &caller, ws, "temp")
        .await
        .unwrap();
    assert!(resolve_persona(&node, &caller, ws, None)
        .await
        .expect("no error for a dangling member default")
        .is_none());

    // Explicit unknown id → named error (an explicit ask must not silently degrade).
    assert!(matches!(
        resolve_persona(&node, &caller, ws, Some("ghost")).await,
        Err(ToolError::NotFound)
    ));
}

// ================================================================================================
// Roster semantics (host)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn roster_none_means_all_enabled_and_some_filters_the_list() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.expect("seed");
    let ws = "sess-roster";
    let admin = principal("user:ada", ws, &[P_LIST, P_CREATE, CFG_SET]);
    agent_persona_create(&node, &admin, ws, &persona("local", "agent.memory.list"))
        .await
        .unwrap();

    // No roster → every persona (built-ins + the custom) lists `enabled: true`.
    let list = agent_persona_list(&node, &admin, ws).await.expect("list");
    assert!(list.len() > 2, "built-ins + custom present");
    assert!(list.iter().all(|p| p.enabled), "None roster = all enabled");

    // Roster set → only the named ids are enabled; the rest still LIST (the Settings editor needs
    // them) but flagged disabled. A deleted/unknown id in the roster is inert.
    set_roster(
        &node,
        &admin,
        ws,
        Some(vec!["builtin.flow-author", "no-such-persona"]),
    )
    .await;
    let list = agent_persona_list(&node, &admin, ws).await.expect("list");
    let flow = list.iter().find(|p| p.id == "builtin.flow-author").unwrap();
    let local = list.iter().find(|p| p.id == "local").unwrap();
    assert!(flow.enabled);
    assert!(!local.enabled);
    assert!(
        !list.iter().all(|p| p.enabled),
        "a Some roster disables the unnamed"
    );

    // Empty roster = cleared → back to all enabled (the MERGE-can't-null workaround).
    set_roster(&node, &admin, ws, Some(vec![])).await;
    let list = agent_persona_list(&node, &admin, ws).await.expect("list");
    assert!(list.iter().all(|p| p.enabled), "empty roster clears");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_explicit_invoke_of_a_disabled_persona_fails_named_and_a_disabled_default_folds_to_none()
{
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "sess-disabled";
    let admin = principal("user:ada", ws, &[P_CREATE, CFG_SET]);
    agent_persona_create(&node, &admin, ws, &persona("on", "agent.memory.list"))
        .await
        .unwrap();
    agent_persona_create(&node, &admin, ws, &persona("off", "agent.memory.get"))
        .await
        .unwrap();
    set_roster(&node, &admin, ws, Some(vec!["on"])).await;

    // Enabled explicit id still resolves.
    assert_eq!(
        resolve_persona(&node, &admin, ws, Some("on"))
            .await
            .unwrap()
            .unwrap()
            .id,
        "on"
    );
    // Disabled explicit id → named BadInput carrying the reason (curation working as intended).
    match resolve_persona(&node, &admin, ws, Some("off")).await {
        Err(ToolError::BadInput(msg)) => {
            assert!(
                msg.contains("disabled"),
                "the error names the curation: {msg}"
            )
        }
        other => panic!("expected the named disabled error, got {other:?}"),
    }
    // A disabled DEFAULT folds to none (warn), never an errored run.
    set_workspace_prefs(
        &node.store,
        ws,
        &Prefs {
            agent_persona: Some("off".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(resolve_persona(&node, &admin, ws, None)
        .await
        .expect("no error")
        .is_none());
}

// ================================================================================================
// Workspace isolation (mandatory)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_a_default_and_roster_never_affect_a_ws_b_run() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let a = principal("user:ada", "sess-iso-a", &[P_CREATE, CFG_SET]);
    let b = principal("user:bo", "sess-iso-b", &[P_CREATE]);
    agent_persona_create(
        &node,
        &a,
        "sess-iso-a",
        &persona("a-only", "agent.memory.list"),
    )
    .await
    .unwrap();
    agent_persona_create(
        &node,
        &b,
        "sess-iso-b",
        &persona("b-own", "agent.memory.get"),
    )
    .await
    .unwrap();

    // ws-A: member default + a restrictive roster. ws-B: nothing.
    set_user_prefs(
        &node.store,
        "sess-iso-a",
        "user:ada",
        &Prefs {
            agent_persona: Some("a-only".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    set_roster(&node, &a, "sess-iso-a", Some(vec!["a-only"])).await;

    // ws-B resolves none (A's default/roster invisible) and its own explicit id fine.
    assert!(resolve_persona(&node, &b, "sess-iso-b", None)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        resolve_persona(&node, &b, "sess-iso-b", Some("b-own"))
            .await
            .unwrap()
            .unwrap()
            .id,
        "b-own"
    );

    // ws-A's custom id stored as ws-B's default is UNRESOLVABLE from ws-B: folds to none + warn.
    set_workspace_prefs(
        &node.store,
        "sess-iso-b",
        &Prefs {
            agent_persona: Some("a-only".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    assert!(
        resolve_persona(&node, &b, "sess-iso-b", None)
            .await
            .expect("no error")
            .is_none(),
        "a ws-A custom persona never crosses the wall as a ws-B default"
    );
}

// ================================================================================================
// Capability deny (mandatory — one per verb touched)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_is_denied_roster_and_ws_default_writes_but_allowed_own_default() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "sess-deny";
    let member = principal("user:mia", ws, &[PREFS_SET]); // no CFG_SET, no PREFS_SET_DEFAULT

    // Roster write denied at the wall.
    assert!(matches!(
        agent_config_set(
            &node,
            &member,
            ws,
            &AgentConfig {
                enabled_personas: Some(vec!["builtin.flow-author".into()]),
                ..Default::default()
            },
        )
        .await,
        Err(ToolError::Denied)
    ));

    // ws-default persona write denied.
    assert!(matches!(
        prefs_set_default(
            &node.store,
            &member,
            ws,
            &Prefs {
                agent_persona: Some("builtin.flow-author".into()),
                ..Default::default()
            },
        )
        .await,
        Err(PrefsSvcError::Denied)
    ));

    // Own member default allowed — and it round-trips into resolution.
    let caller = principal("user:mia", ws, &[PREFS_SET, P_CREATE]);
    agent_persona_create(&node, &caller, ws, &persona("mine", "agent.memory.list"))
        .await
        .unwrap();
    prefs_set(
        &node.store,
        &member,
        ws,
        &Prefs {
            agent_persona: Some("mine".into()),
            ..Default::default()
        },
    )
    .await
    .expect("member may set their own default");
    assert_eq!(
        resolve_persona(&node, &member, ws, None)
            .await
            .unwrap()
            .unwrap()
            .id,
        "mine"
    );
}

// ================================================================================================
// Independence (the headline)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_members_and_two_explicit_ids_resolve_independently_with_zero_server_session_state() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "sess-independent";
    let ada = principal("user:ada", ws, &[P_CREATE]);
    let bo = principal("user:bo", ws, &[]);
    agent_persona_create(&node, &ada, ws, &persona("p-list", "agent.memory.list"))
        .await
        .unwrap();
    agent_persona_create(&node, &ada, ws, &persona("p-get", "agent.memory.get"))
        .await
        .unwrap();

    // Two members, two member defaults, one workspace — resolved in the same tick.
    set_user_prefs(
        &node.store,
        ws,
        "user:ada",
        &Prefs {
            agent_persona: Some("p-list".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    set_user_prefs(
        &node.store,
        ws,
        "user:bo",
        &Prefs {
            agent_persona: Some("p-get".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let (ra, rb) = tokio::join!(
        resolve_persona(&node, &ada, ws, None),
        resolve_persona(&node, &bo, ws, None)
    );
    assert_eq!(ra.unwrap().unwrap().id, "p-list");
    assert_eq!(rb.unwrap().unwrap().id, "p-get");

    // One member, two explicit ids back to back (the per-tab pin as pure invoke args) — both
    // narrow correctly, no server write in between.
    let (r1, r2) = tokio::join!(
        resolve_persona(&node, &ada, ws, Some("p-get")),
        resolve_persona(&node, &ada, ws, Some("p-list"))
    );
    assert_eq!(r1.unwrap().unwrap().id, "p-get");
    assert_eq!(r2.unwrap().unwrap().id, "p-list");
}

// ================================================================================================
// Migration (legacy active_persona → ws-default prefs axis)
// ================================================================================================

/// Plant a legacy `active_persona` value the way an old node wrote it (raw column — the field is
/// decode-only now, so the gated verb can no longer write it).
async fn plant_legacy(node: &Arc<Node>, ws: &str, id: &str) {
    node.store
        .query_ws(
            ws,
            "UPSERT type::thing('workspace_agent_config', [$ws]) MERGE { ws: $ws, active_persona: $id }",
            vec![
                ("ws".into(), Value::String(ws.into())),
                ("id".into(), Value::String(id.into())),
            ],
        )
        .await
        .expect("legacy write");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn migration_copies_the_legacy_toggle_once_idempotently_and_never_clobbers_an_admin_write() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    // Three known workspaces (the reactor directory is one of `known_workspaces`' sources).
    for ws in ["mig-a", "mig-b", "mig-c"] {
        register_workspace(&node.store, ws, "general", 1)
            .await
            .unwrap();
    }
    // mig-a: legacy set, no ws default → migrates. mig-b: legacy set AND admin-set axis → the admin
    // write wins, the legacy is still cleared. mig-c: nothing → nothing appears.
    plant_legacy(&node, "mig-a", "builtin.flow-author").await;
    plant_legacy(&node, "mig-b", "builtin.data-analyst").await;
    set_workspace_prefs(
        &node.store,
        "mig-b",
        &Prefs {
            agent_persona: Some("builtin.system-manager".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();

    let migrated = migrate_active_persona(&node.store).await.expect("migrates");
    assert!(migrated.contains(&"mig-a".to_string()));
    assert!(migrated.contains(&"mig-b".to_string()));
    assert!(!migrated.contains(&"mig-c".to_string()));

    async fn axis(node: &Node, ws: &str) -> Option<String> {
        get_workspace_prefs(&node.store, ws)
            .await
            .unwrap()
            .and_then(|p| p.agent_persona)
    }
    assert_eq!(
        axis(&node, "mig-a").await.as_deref(),
        Some("builtin.flow-author")
    );
    assert_eq!(
        axis(&node, "mig-b").await.as_deref(),
        Some("builtin.system-manager"),
        "an admin-set axis is never overwritten"
    );
    assert_eq!(
        axis(&node, "mig-c").await,
        None,
        "unset legacy ⇒ no ws default appears"
    );

    // Second boot: nothing left to copy (the legacy column was cleared) — idempotent.
    let second = migrate_active_persona(&node.store).await.expect("re-runs");
    assert!(
        second.is_empty(),
        "a second boot migrates nothing: {second:?}"
    );
    assert_eq!(
        axis(&node, "mig-a").await.as_deref(),
        Some("builtin.flow-author")
    );
}

// ================================================================================================
// Surfaces as data (the record half of the rule-10 swap test)
// ================================================================================================

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_custom_persona_with_a_novel_surface_is_a_record_edit_only() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_personas(&node.store).await.expect("seed");
    let ws = "sess-surfaces";
    let admin = principal("user:ada", ws, &[P_CREATE, P_LIST]);

    // A NOVEL surface string no built-in claims and no code knows.
    let mut p = persona("studio", "agent.memory.list");
    p.surfaces = vec!["holodeck".into()];
    agent_persona_create(&node, &admin, ws, &p)
        .await
        .expect("create");

    // It round-trips through the one list fetch the dock matches over — zero code change. (The
    // matching itself is client-side; its gateway test drives a dock pointed at the surface.)
    let list = agent_persona_list(&node, &admin, ws).await.expect("list");
    let studio = list.iter().find(|q| q.id == "studio").expect("listed");
    assert_eq!(studio.surfaces, vec!["holodeck".to_string()]);
    assert!(studio.enabled);
    // Built-ins ship their surfaces the same way (data from personas.toml).
    let flow = list
        .iter()
        .find(|q| q.id == "builtin.flow-author")
        .expect("built-in");
    assert!(flow.surfaces.contains(&"flows".to_string()));
}
