//! `agent.def.*` — the agent-definition catalog (agent-catalog scope). Boots a REAL `Node` (no mocks;
//! testing §0 — store + registry + gate all real), seeds the built-ins through the real boot seeder,
//! and drives the five verbs. Covers:
//!   - SEED + node-runnable FILTER: the seeder writes the nine built-ins into `_lb_agents`; `list`
//!     returns the in-house six (the `default` runtime the node always offers — three coding plus three
//!     general-purpose) and OMITS the open-interpreter three (the feature/runtime is not offered in this
//!     test build);
//!   - SEED IDEMPOTENCY: a second seed is a no-op in effect (no duplicates; same catalog);
//!   - CAPABILITY-DENY, per verb (§2.1, opaque): each verb denied without its own cap;
//!   - READ-ONLY BUILT-IN TIER: create/update/delete of a `builtin.*` id is `BadInput` (reserved),
//!     BEFORE the caps gate (an admin holding every cap still cannot);
//!   - CUSTOM CRUD round-trip: create → list/get → update → delete;
//!   - RUNTIME VALIDATION: a create/update naming an unoffered runtime is `BadInput`;
//!   - WORKSPACE-ISOLATION (§2.2): a ws-B admin cannot get/update/delete a ws-A custom definition;
//!   - OFFLINE/SYNC: a double-applied create upserts idempotently (LWW on the slug);
//!   - NAMES-ONLY: a definition carries `api_key_env` (a NAME), never a secret value.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    agent_def_create, agent_def_delete, agent_def_get, agent_def_list, agent_def_update,
    seed_agent_definitions, AgentDefinition, DefinitionEndpoint, DefinitionPatch, Node,
};
use lb_mcp::ToolError;

const LIST: &str = "mcp:agent.def.list:call";
const GET: &str = "mcp:agent.def.get:call";
const CREATE: &str = "mcp:agent.def.create:call";
const UPDATE: &str = "mcp:agent.def.update:call";
const DELETE: &str = "mcp:agent.def.delete:call";

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

/// A member holding every read+write cap (an "admin" for these tests).
fn admin(sub: &str, ws: &str) -> Principal {
    principal(sub, ws, &[LIST, GET, CREATE, UPDATE, DELETE])
}

/// A sample custom definition over the always-present `default` runtime, names-only endpoint.
fn sample_custom(id: &str) -> AgentDefinition {
    AgentDefinition {
        id: id.into(),
        label: "Custom — GLM-5.2 (staging)".into(),
        description: Some("A staging-key custom preset.".into()),
        runtime: "default".into(),
        model_endpoint: DefinitionEndpoint {
            provider: "zaicoding".into(),
            model: "glm-5.2".into(),
            api_key_env: Some("ZAI_STAGING_KEY".into()),
            api_key_secret: None,
            base_url: Some("https://api.z.ai/api/coding/paas/v4".into()),
        },
        builtin: false,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seed_lists_node_runnable_builtins_and_filters_the_rest() {
    let node = Node::boot().await.expect("node boots");
    let seeded = seed_agent_definitions(&node.store).await.expect("seed");
    assert_eq!(
        seeded.len(),
        9,
        "nine built-ins seeded (6 in-house coding/general + 3 open-interpreter)"
    );

    let ws = "defs-seed";
    let p = admin("user:ada", ws);
    let list = agent_def_list(&node, &p, ws).await.expect("list");

    // The in-house six (runtime `default`, always offered) list — three coding (glm-4.6/5.1/5.2) and
    // three general-purpose (glm-4.5-air/4.5/5-turbo); the open-interpreter three do NOT (their runtime
    // is not registered in this test build) — the node-runnable filter, symmetric.
    let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
    assert!(
        ids.contains(&"builtin.in-house-glm-4.6"),
        "in-house built-in lists: {ids:?}"
    );
    assert!(ids.contains(&"builtin.in-house-glm-5.1"));
    assert!(ids.contains(&"builtin.in-house-glm-5.2"));
    assert!(
        ids.contains(&"builtin.in-house-glm-4.5-air")
            && ids.contains(&"builtin.in-house-glm-4.5")
            && ids.contains(&"builtin.in-house-glm-5-turbo"),
        "the general-purpose in-house built-ins list: {ids:?}"
    );
    assert!(
        !ids.iter()
            .any(|id| id.starts_with("builtin.open-interpreter")),
        "open-interpreter built-ins are seeded but filtered (runtime not offered): {ids:?}"
    );
    assert!(
        list.iter().all(|d| d.builtin),
        "all listed here are built-ins (no custom yet)"
    );

    // NAMES-ONLY: the seeded endpoint carries the env NAME, never a secret value.
    let inhouse = list
        .iter()
        .find(|d| d.id == "builtin.in-house-glm-4.6")
        .unwrap();
    assert_eq!(
        inhouse.model_endpoint.api_key_env.as_deref(),
        Some("ZAI_API_KEY")
    );
    let json = serde_json::to_string(&list).unwrap();
    assert!(
        !json.to_lowercase().contains("sk-"),
        "no secret value in the catalog: {json}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seed_is_idempotent() {
    let node = Node::boot().await.expect("node boots");
    seed_agent_definitions(&node.store).await.expect("seed 1");
    seed_agent_definitions(&node.store)
        .await
        .expect("seed 2 (replay)");

    let ws = "defs-idem";
    let p = admin("user:ada", ws);
    let list = agent_def_list(&node, &p, ws).await.expect("list");
    let builtins = list.iter().filter(|d| d.builtin).count();
    assert_eq!(
        builtins, 6,
        "a re-seed introduces no duplicate built-ins (LWW upsert)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let node = Node::boot().await.expect("node boots");
    seed_agent_definitions(&node.store).await.expect("seed");
    let ws = "defs-deny";

    // A member with NO caps at all.
    let none = principal("user:eve", ws, &[]);
    assert!(matches!(
        agent_def_list(&node, &none, ws).await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        agent_def_get(&node, &none, ws, "builtin.in-house-glm-4.6").await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        agent_def_create(&node, &none, ws, &sample_custom("x")).await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        agent_def_update(&node, &none, ws, "x", DefinitionPatch::default()).await,
        Err(ToolError::Denied)
    ));
    assert!(matches!(
        agent_def_delete(&node, &none, ws, "x").await,
        Err(ToolError::Denied)
    ));

    // A member with the READ caps but no writes: list/get succeed, every write is denied.
    let reader = principal("user:mem", ws, &[LIST, GET]);
    assert!(agent_def_list(&node, &reader, ws).await.is_ok());
    assert!(
        agent_def_get(&node, &reader, ws, "builtin.in-house-glm-4.6")
            .await
            .is_ok()
    );
    assert!(matches!(
        agent_def_create(&node, &reader, ws, &sample_custom("y")).await,
        Err(ToolError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn builtin_ids_are_read_only_even_for_an_admin() {
    let node = Node::boot().await.expect("node boots");
    seed_agent_definitions(&node.store).await.expect("seed");
    let ws = "defs-readonly";
    let p = admin("user:ada", ws);

    // create/update/delete of a `builtin.*` id → BadInput (reserved), checked BEFORE the caps gate.
    let mut builtin = sample_custom("builtin.in-house-glm-4.6");
    builtin.id = "builtin.in-house-glm-4.6".into();
    assert!(matches!(
        agent_def_create(&node, &p, ws, &builtin).await,
        Err(ToolError::BadInput(_))
    ));
    assert!(matches!(
        agent_def_update(
            &node,
            &p,
            ws,
            "builtin.in-house-glm-4.6",
            DefinitionPatch {
                label: Some("hacked".into()),
                ..DefinitionPatch::default()
            }
        )
        .await,
        Err(ToolError::BadInput(_))
    ));
    assert!(matches!(
        agent_def_delete(&node, &p, ws, "builtin.in-house-glm-4.6").await,
        Err(ToolError::BadInput(_))
    ));

    // The built-in still reads cleanly (unchanged).
    let got = agent_def_get(&node, &p, ws, "builtin.in-house-glm-4.6")
        .await
        .expect("get");
    assert_eq!(got.label, "In-house — Z.AI GLM-4.6");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn custom_crud_round_trips() {
    let node = Node::boot().await.expect("node boots");
    seed_agent_definitions(&node.store).await.expect("seed");
    let ws = "defs-crud";
    let p = admin("user:ada", ws);

    // Create.
    agent_def_create(&node, &p, ws, &sample_custom("staging-glm-5.2"))
        .await
        .expect("create");

    // It lists (after the built-ins) and gets, tagged custom.
    let list = agent_def_list(&node, &p, ws).await.expect("list");
    let custom = list
        .iter()
        .find(|d| d.id == "staging-glm-5.2")
        .expect("custom lists");
    assert!(!custom.builtin);
    let got = agent_def_get(&node, &p, ws, "staging-glm-5.2")
        .await
        .expect("get");
    assert_eq!(got.model_endpoint.model, "glm-5.2");
    assert!(!got.builtin);

    // Update the label + endpoint model.
    agent_def_update(
        &node,
        &p,
        ws,
        "staging-glm-5.2",
        DefinitionPatch {
            label: Some("Custom — GLM-5.1 (staging)".into()),
            model_endpoint: Some(DefinitionEndpoint {
                provider: "zaicoding".into(),
                model: "glm-5.1".into(),
                api_key_env: Some("ZAI_STAGING_KEY".into()),
                api_key_secret: None,
                base_url: None,
            }),
            ..DefinitionPatch::default()
        },
    )
    .await
    .expect("update");
    let got = agent_def_get(&node, &p, ws, "staging-glm-5.2")
        .await
        .expect("get");
    assert_eq!(got.label, "Custom — GLM-5.1 (staging)");
    assert_eq!(got.model_endpoint.model, "glm-5.1");

    // Delete → gone (NotFound on get).
    agent_def_delete(&node, &p, ws, "staging-glm-5.2")
        .await
        .expect("delete");
    assert!(matches!(
        agent_def_get(&node, &p, ws, "staging-glm-5.2").await,
        Err(ToolError::NotFound)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unknown_runtime_on_write_is_rejected() {
    let node = Node::boot().await.expect("node boots");
    let ws = "defs-runtime";
    let p = admin("user:ada", ws);

    let mut bad = sample_custom("bad-runtime");
    bad.runtime = "no-such-runtime".into();
    assert!(
        matches!(
            agent_def_create(&node, &p, ws, &bad).await,
            Err(ToolError::BadInput(_))
        ),
        "a runtime the node cannot run is a BadInput, not a silent accept"
    );

    // A valid create then an update to an unoffered runtime is also rejected.
    agent_def_create(&node, &p, ws, &sample_custom("ok"))
        .await
        .expect("create ok");
    assert!(matches!(
        agent_def_update(
            &node,
            &p,
            ws,
            "ok",
            DefinitionPatch {
                runtime: Some("no-such-runtime".into()),
                ..DefinitionPatch::default()
            }
        )
        .await,
        Err(ToolError::BadInput(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn custom_definitions_are_workspace_isolated() {
    let node = Node::boot().await.expect("node boots");
    let admin_a = admin("user:ada", "ws-a");
    let admin_b = admin("user:bob", "ws-b");

    agent_def_create(&node, &admin_a, "ws-a", &sample_custom("secret-a"))
        .await
        .expect("ws-a create");

    // ws-B cannot see, edit, or delete ws-A's custom definition — the hard wall.
    assert!(matches!(
        agent_def_get(&node, &admin_b, "ws-b", "secret-a").await,
        Err(ToolError::NotFound)
    ));
    assert!(matches!(
        agent_def_update(
            &node,
            &admin_b,
            "ws-b",
            "secret-a",
            DefinitionPatch {
                label: Some("stolen".into()),
                ..DefinitionPatch::default()
            }
        )
        .await,
        Err(ToolError::NotFound)
    ));
    // Delete is idempotent (no-op on absent) but must NOT touch ws-A's record.
    agent_def_delete(&node, &admin_b, "ws-b", "secret-a")
        .await
        .expect("ws-b delete is a no-op");
    assert!(
        agent_def_get(&node, &admin_a, "ws-a", "secret-a")
            .await
            .is_ok(),
        "ws-A's definition is untouched by a ws-B delete"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn double_create_is_idempotent() {
    let node = Node::boot().await.expect("node boots");
    let ws = "defs-offline";
    let p = admin("user:ada", ws);

    agent_def_create(&node, &p, ws, &sample_custom("dup"))
        .await
        .expect("first");
    agent_def_create(&node, &p, ws, &sample_custom("dup"))
        .await
        .expect("replay");

    let list = agent_def_list(&node, &p, ws).await.expect("list");
    let count = list.iter().filter(|d| d.id == "dup").count();
    assert_eq!(
        count, 1,
        "a replayed create upserts the same slug (no duplicate, LWW)"
    );
}
