//! Agent-memory scope — durable, access-walled agent memory (headless, rule 9: real `mem://` store,
//! real gateway, real loop; no fakes). Covers the mandatory categories + the decided bounds/lint.
//!
//! Mandatory (testing §2):
//!   - **capability-deny (per verb)**: no cap → denied; `set` with the verb cap but WITHOUT the
//!     workspace-scope write gate → workspace `set` denied while member `set` succeeds.
//!   - **workspace-isolation**: ws-B lists/gets nothing of ws-A (store + MCP).
//!   - **member wall**: bob's resolution never returns `member:ada` rows even with slugs known — the
//!     scope is derived from the principal, asserted directly.
//!   - **offline/sync**: a double-applied `set` is idempotent (composite id, LWW).
//!   - **injection (real run)**: a real in-house run's context contains the index after a `set`,
//!     loses it after `delete`.

use std::sync::{Arc, Mutex};

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_agent_memory_tool, invoke, memory_get, memory_list, memory_set, AllowedTool, Invocation,
    Node,
};
use lb_mcp::ToolError;
use lb_role_ai_gateway::{AiGateway, AiRequest, AiResponse, Provider};
use lb_store::Store;
use serde_json::json;

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
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const LIST: &str = "mcp:agent.memory.list:call";
const GET: &str = "mcp:agent.memory.get:call";
const SET: &str = "mcp:agent.memory.set:call";
const DELETE: &str = "mcp:agent.memory.delete:call";
const WS_WRITE: &str = "store:agent_memory/workspace:write";

/// The full member cap set (all four verbs, member scope only — NO workspace-scope write).
const MEMBER: &[&str] = &[LIST, GET, SET, DELETE];

// ── capability deny, per verb ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_denied_without_its_cap() {
    let ws = "ws-mem-deny";
    let store = Store::memory().await.unwrap();
    let nobody = principal("user:ada", ws, &[]); // no caps

    assert!(memory_list(&store, &nobody, ws).await.is_err());
    assert!(memory_get(&store, &nobody, ws, None, "s").await.is_err());
    assert!(
        memory_set(&store, &nobody, ws, None, "s", "d", "user", "b", 1)
            .await
            .is_err()
    );
    assert!(memory_list(&store, &nobody, ws).await.is_err());
    // Holding only `list` does NOT grant `set`.
    let list_only = principal("user:ada", ws, &[LIST]);
    assert!(matches!(
        memory_set(&store, &list_only, ws, None, "s", "d", "user", "b", 1)
            .await
            .unwrap_err(),
        ToolError::Denied
    ));
}

// ── the workspace-scope write gate is distinct from the member write ─────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn member_set_ok_but_workspace_set_needs_the_ws_write_gate() {
    let ws = "ws-mem-wsgate";
    let store = Store::memory().await.unwrap();
    // Has the verb caps but NOT the workspace-scope write gate.
    let member = principal("user:ada", ws, MEMBER);

    // Member-scope set succeeds (a run may always curate its own member memory).
    memory_set(
        &store,
        &member,
        ws,
        Some("member"),
        "terse",
        "d",
        "user",
        "be terse",
        1,
    )
    .await
    .unwrap();
    // Workspace-scope set is DENIED — needs `store:agent_memory/workspace:write`.
    assert!(matches!(
        memory_set(
            &store,
            &member,
            ws,
            Some("workspace"),
            "house-rule",
            "d",
            "project",
            "b",
            1
        )
        .await
        .unwrap_err(),
        ToolError::Denied
    ));

    // With the ws-write gate, the workspace set succeeds.
    let mut caps = MEMBER.to_vec();
    caps.push(WS_WRITE);
    let curator = principal("user:ada", ws, &caps);
    memory_set(
        &store,
        &curator,
        ws,
        Some("workspace"),
        "house-rule",
        "d",
        "project",
        "b",
        1,
    )
    .await
    .unwrap();
}

// ── the member wall: a run under bob NEVER sees member:ada ───────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_can_never_read_another_members_memory() {
    let ws = "ws-mem-wall";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, MEMBER);
    let bob = principal("user:bob", ws, MEMBER);

    // Ada writes a private member fact.
    memory_set(
        &store,
        &ada,
        ws,
        Some("member"),
        "ada-secret",
        "ada only",
        "user",
        "xyz",
        1,
    )
    .await
    .unwrap();

    // Bob's list resolves `workspace + member:bob` — never `member:ada`.
    let bobs = memory_list(&store, &bob, ws).await.unwrap();
    assert!(
        bobs.iter().all(|m| m.slug != "ada-secret"),
        "bob's list never returns ada's member row"
    );

    // Even knowing the slug, bob's `get member` binds to member:bob — ada's fact is NOT returned.
    assert!(
        memory_get(&store, &bob, ws, Some("member"), "ada-secret")
            .await
            .unwrap()
            .is_none(),
        "bob cannot get ada's member fact by slug (scope is derived from the principal)"
    );
    // Ada herself sees it (sanity — the fact exists, it's just walled).
    assert!(memory_get(&store, &ada, ws, Some("member"), "ada-secret")
        .await
        .unwrap()
        .is_some());
}

// ── workspace isolation ──────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_sees_nothing_of_ws_a() {
    let store = Store::memory().await.unwrap();
    let ada_a = principal("user:ada", "ws-a", MEMBER);
    memory_set(
        &store,
        &ada_a,
        "ws-a",
        Some("member"),
        "a-fact",
        "d",
        "user",
        "b",
        1,
    )
    .await
    .unwrap();

    // Same user id, different workspace → a different namespace. Nothing bleeds across.
    let ada_b = principal("user:ada", "ws-b", MEMBER);
    assert!(memory_list(&store, &ada_b, "ws-b")
        .await
        .unwrap()
        .is_empty());
    assert!(memory_get(&store, &ada_b, "ws-b", Some("member"), "a-fact")
        .await
        .unwrap()
        .is_none());
}

// ── idempotent upsert (LWW) + one index row ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_is_an_idempotent_upsert() {
    let ws = "ws-mem-upsert";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, MEMBER);

    memory_set(
        &store,
        &ada,
        ws,
        Some("member"),
        "pref",
        "v1",
        "user",
        "body1",
        1,
    )
    .await
    .unwrap();
    // Second set, same {scope, slug} → replaces (LWW), one row.
    memory_set(
        &store,
        &ada,
        ws,
        Some("member"),
        "pref",
        "v2",
        "user",
        "body2",
        2,
    )
    .await
    .unwrap();

    let rows = memory_list(&store, &ada, ws).await.unwrap();
    assert_eq!(rows.iter().filter(|m| m.slug == "pref").count(), 1);
    let got = memory_get(&store, &ada, ws, Some("member"), "pref")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.description, "v2");
    assert_eq!(got.body, "body2");
    assert_eq!(got.updated_at, 2);
}

// ── bounds + secret lint ─────────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_enforces_bounds_and_the_secret_lint() {
    let ws = "ws-mem-bounds";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, MEMBER);

    // description > 120 chars.
    let long_desc = "x".repeat(121);
    assert!(matches!(
        memory_set(&store, &ada, ws, None, "s", &long_desc, "user", "b", 1)
            .await
            .unwrap_err(),
        ToolError::BadInput(_)
    ));
    // body > 8 KB.
    let big_body = "y".repeat(8 * 1024 + 1);
    assert!(matches!(
        memory_set(&store, &ada, ws, None, "s", "d", "user", &big_body, 1)
            .await
            .unwrap_err(),
        ToolError::BadInput(_)
    ));
    // unknown kind.
    assert!(matches!(
        memory_set(&store, &ada, ws, None, "s", "d", "bogus", "b", 1)
            .await
            .unwrap_err(),
        ToolError::BadInput(_)
    ));
    // secret lint: an assigned credential shape is refused.
    assert!(matches!(
        memory_set(
            &store,
            &ada,
            ws,
            None,
            "s",
            "d",
            "reference",
            "password: hunter2xyz",
            1
        )
        .await
        .unwrap_err(),
        ToolError::BadInput(_)
    ));
    // an sk- key is refused.
    assert!(memory_set(
        &store,
        &ada,
        ws,
        None,
        "s",
        "d",
        "reference",
        "the key is sk-abcdefghij0123456789",
        1
    )
    .await
    .is_err());
    // a benign fact that merely mentions the word "password" in prose is fine.
    memory_set(
        &store,
        &ada,
        ws,
        None,
        "s",
        "reset your password via the console",
        "reference",
        "Users reset a password through Settings.",
        1,
    )
    .await
    .unwrap();
}

// ── over the MCP bridge: list/get/set/delete + per-verb MCP deny ─────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_mcp_surface_roundtrips_and_denies_per_verb() {
    let ws = "ws-mem-mcp";
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", ws, MEMBER);

    // set → get → list → delete over the bridge.
    call_agent_memory_tool(
        &store,
        &ada,
        ws,
        "agent.memory.set",
        &json!({"scope":"member","slug":"pref","description":"terse","kind":"user","body":"be terse","ts":1}),
    )
    .await
    .unwrap()
    .unwrap();
    let got = call_agent_memory_tool(
        &store,
        &ada,
        ws,
        "agent.memory.get",
        &json!({"slug":"pref"}),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(got["body"], "be terse");
    let list = call_agent_memory_tool(&store, &ada, ws, "agent.memory.list", &json!({}))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(list["memories"].as_array().unwrap().len(), 1);
    // list rows never carry the body (loaded on demand by get).
    assert!(list["memories"][0].get("body").is_none());

    call_agent_memory_tool(
        &store,
        &ada,
        ws,
        "agent.memory.delete",
        &json!({"slug":"pref"}),
    )
    .await
    .unwrap()
    .unwrap();
    let list = call_agent_memory_tool(&store, &ada, ws, "agent.memory.list", &json!({}))
        .await
        .unwrap()
        .unwrap();
    assert!(list["memories"].as_array().unwrap().is_empty());

    // per-verb deny at the MCP gate.
    let nobody = principal("user:ada", ws, &[]);
    for (verb, args) in [
        ("agent.memory.list", json!({})),
        ("agent.memory.get", json!({"slug":"pref"})),
        (
            "agent.memory.set",
            json!({"slug":"s","description":"d","kind":"user","body":"b","ts":1}),
        ),
        ("agent.memory.delete", json!({"slug":"pref"})),
    ] {
        let err = call_agent_memory_tool(&store, &nobody, ws, verb, &args)
            .await
            .unwrap()
            .unwrap_err();
        assert!(
            matches!(err, ToolError::Denied),
            "{verb} must deny, got {err:?}"
        );
    }
}

// ── real in-house run: the memory index is injected, and tracks set/delete ───────────────────────

struct CapturingProvider {
    seen: Arc<Mutex<Vec<String>>>,
}

impl Provider for CapturingProvider {
    async fn complete(&self, req: &AiRequest) -> AiResponse {
        let joined = req
            .messages
            .iter()
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");
        *self.seen.lock().unwrap() = vec![joined];
        AiResponse::stop("done", 1)
    }
}

async fn run_ctx(node: &Arc<Node>, caller: &Principal, ws: &str, job: &str) -> String {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let gw = AiGateway::new(CapturingProvider { seen: seen.clone() });
    invoke(
        node,
        &gw,
        caller,
        &[
            "mcp:agent.memory.list:call".into(),
            "mcp:agent.memory.get:call".into(),
        ],
        ws,
        Invocation {
            job_id: job,
            goal: "hi",
            skill: None,
            doc: None,
            tools: &[AllowedTool {
                name: "noop".into(),
                description: "".into(),
                input_schema: None,
            }],
            ts: 1,
        },
    )
    .await
    .expect("run completes");
    let captured = seen.lock().unwrap().first().cloned().unwrap_or_default();
    captured
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_real_run_injects_the_memory_index_after_set_and_loses_it_after_delete() {
    let ws = "ws-mem-inject";
    let node = Arc::new(Node::boot().await.unwrap());
    let caller = principal(
        "user:ada",
        ws,
        &["mcp:agent.invoke:call", LIST, GET, SET, DELETE],
    );

    // No memory yet → no index in context.
    let ctx0 = run_ctx(&node, &caller, ws, "m-0").await;
    assert!(!ctx0.contains("staging-db-readonly"));

    // Set a workspace fact (caller holds the ws-write gate? member scope avoids needing it).
    memory_set(
        &node.store,
        &caller,
        ws,
        Some("member"),
        "staging-db-readonly",
        "staging DB is a read replica — never write to it",
        "project",
        "Staging mirrors prod read-only.",
        1,
    )
    .await
    .unwrap();

    // Next run's context carries the index line (recalled background), framed, with the description.
    let ctx1 = run_ctx(&node, &caller, ws, "m-1").await;
    assert!(
        ctx1.contains("staging-db-readonly"),
        "index injected after set"
    );
    assert!(
        ctx1.contains("Recalled memory"),
        "framed as recalled background, not instructions"
    );
    assert!(
        ctx1.contains("read replica"),
        "the description is in the index line"
    );

    // Delete → the next run loses it.
    lb_host::memory_delete(
        &node.store,
        &caller,
        ws,
        Some("member"),
        "staging-db-readonly",
    )
    .await
    .unwrap();
    let ctx2 = run_ctx(&node, &caller, ws, "m-2").await;
    assert!(
        !ctx2.contains("staging-db-readonly"),
        "index gone after delete"
    );
}
