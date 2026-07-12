//! `agent.config.get` / `agent.config.set` — the per-workspace agent-config record (agent-config
//! scope). Boots a REAL `Node` (no mocks; testing §0 — store + registry + gate all real) and covers:
//!   - ROUND-TRIP: an admin `set` then `get` echoes the patch (default_runtime + names-only endpoint);
//!   - CAPABILITY-DENY (opaque, §2.1): `set` without `mcp:agent.config.set:call` → `Denied`;
//!     `get` without `mcp:agent.config.get:call` → `Denied` (no leak);
//!   - WORKSPACE-ISOLATION (§2.2, specified): ws-A and ws-B set DIFFERENT runtimes; a `get` in ws-B
//!     returns ws-B's value and NEVER ws-A's, and a ws-A `set` does not move ws-B;
//!   - REGISTRY VALIDATION: a `set` naming a runtime the node does not offer is `BadInput`, not a
//!     silent accept;
//!   - OFFLINE/SYNC: a double-applied `set` is idempotent (composite-id UPSERT, LWW) — same record;
//!   - NAMES-ONLY: the stored endpoint carries `api_key_env` (a NAME), never a secret value.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{agent_config_get, agent_config_set, AgentConfig, ModelEndpointPatch, Node};
use lb_mcp::ToolError;

const GET: &str = "mcp:agent.config.get:call";
const SET: &str = "mcp:agent.config.set:call";

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

/// A patch selecting the always-present `default` runtime plus a names-only endpoint.
fn sample_patch() -> AgentConfig {
    AgentConfig {
        compact_budget: None,
        active_definition: None,
        active_persona: None,
        enabled_personas: None,
        default_runtime: Some("default".into()),
        model_endpoint: Some(ModelEndpointPatch {
            provider: Some("zaicoding".into()),
            model: Some("glm-4.6".into()),
            api_key_env: Some("ZAI_API_KEY".into()),
            api_key_secret: None,
            base_url: Some("https://api.z.ai/api/coding/paas/v4".into()),
        }),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_then_get_round_trips_the_patch() {
    let node = Node::boot().await.expect("node boots");
    let ws = "ac-round";
    let admin = principal("user:ada", ws, &[GET, SET]);

    // Unset → None.
    assert!(agent_config_get(&node, &admin, ws)
        .await
        .expect("get")
        .is_none());

    agent_config_set(&node, &admin, ws, &sample_patch())
        .await
        .expect("admin set");

    let got = agent_config_get(&node, &admin, ws)
        .await
        .expect("get")
        .expect("a record now exists");
    assert_eq!(got.default_runtime.as_deref(), Some("default"));
    let json = serde_json::to_string(&got).unwrap();
    let ep = got.model_endpoint.as_ref().expect("endpoint round-trips");
    assert_eq!(ep.provider.as_deref(), Some("zaicoding"));
    assert_eq!(ep.model.as_deref(), Some("glm-4.6"));
    // NAMES-ONLY: the env NAME is stored; no secret value is present anywhere in the record.
    assert_eq!(ep.api_key_env.as_deref(), Some("ZAI_API_KEY"));
    assert!(
        !json.contains("secret") && !json.to_lowercase().contains("apikey\":\"sk-"),
        "no secret value round-trips, only the env-var name: {json}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_without_the_admin_cap_is_denied_opaquely() {
    let node = Node::boot().await.expect("node boots");
    let ws = "ac-deny-set";
    // Holds the read cap but NOT the admin write cap.
    let p = principal("user:eve", ws, &[GET]);

    let err = agent_config_set(&node, &p, ws, &sample_patch())
        .await
        .expect_err("no set cap → denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");

    // And nothing was written (the record stays unset for a reader).
    let admin = principal("user:ada", ws, &[GET, SET]);
    assert!(agent_config_get(&node, &admin, ws)
        .await
        .expect("get")
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn get_without_the_read_cap_is_denied_opaquely() {
    let node = Node::boot().await.expect("node boots");
    let ws = "ac-deny-get";
    let p = principal("user:eve", ws, &[SET]); // has write, not read

    let err = agent_config_get(&node, &p, ws)
        .await
        .expect_err("no read cap → denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspaces_are_isolated() {
    let node = Node::boot().await.expect("node boots");
    let admin_a = principal("user:ada", "ws-a", &[GET, SET]);
    let admin_b = principal("user:bob", "ws-b", &[GET, SET]);

    // Both use the always-present `default` id but DIFFERENT endpoints, so a leak is observable.
    let mut a = sample_patch();
    a.model_endpoint.as_mut().unwrap().model = Some("model-A".into());
    let mut b = sample_patch();
    b.model_endpoint.as_mut().unwrap().model = Some("model-B".into());

    agent_config_set(&node, &admin_a, "ws-a", &a)
        .await
        .expect("set a");
    agent_config_set(&node, &admin_b, "ws-b", &b)
        .await
        .expect("set b");

    let got_a = agent_config_get(&node, &admin_a, "ws-a")
        .await
        .expect("get a")
        .unwrap();
    let got_b = agent_config_get(&node, &admin_b, "ws-b")
        .await
        .expect("get b")
        .unwrap();
    assert_eq!(
        got_a.model_endpoint.unwrap().model.as_deref(),
        Some("model-A")
    );
    assert_eq!(
        got_b.model_endpoint.unwrap().model.as_deref(),
        Some("model-B"),
        "ws-b reads ITS OWN record, never ws-a's"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn setting_an_unknown_runtime_is_rejected() {
    let node = Node::boot().await.expect("node boots");
    let ws = "ac-unknown";
    let admin = principal("user:ada", ws, &[GET, SET]);

    let patch = AgentConfig {
        compact_budget: None,
        active_definition: None,
        active_persona: None,
        enabled_personas: None,
        default_runtime: Some("no-such-runtime".into()),
        model_endpoint: None,
    };
    let err = agent_config_set(&node, &admin, ws, &patch)
        .await
        .expect_err("unknown runtime rejected");
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "an id the node cannot run is a BadInput, not a silent accept: {err:?}"
    );
    // Nothing persisted.
    assert!(agent_config_get(&node, &admin, ws)
        .await
        .expect("get")
        .is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn double_apply_is_idempotent() {
    let node = Node::boot().await.expect("node boots");
    let ws = "ac-idempotent";
    let admin = principal("user:ada", ws, &[GET, SET]);

    agent_config_set(&node, &admin, ws, &sample_patch())
        .await
        .expect("first apply");
    agent_config_set(&node, &admin, ws, &sample_patch())
        .await
        .expect("replay apply");

    let got = agent_config_get(&node, &admin, ws).await.expect("get");
    assert_eq!(
        got.expect("record").default_runtime.as_deref(),
        Some("default"),
        "a replayed set upserts the same composite-id record (no duplicate/LWW drift)"
    );
}
