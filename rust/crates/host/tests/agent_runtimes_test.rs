//! `agent.runtimes` — the run-lifecycle #5 read surface behind the composer runtime picker. Boots a
//! REAL `Node` (no mocks; testing §0 — the registry + store + gate are all real) and exercises the
//! mandatory categories against `list_runtimes` directly, plus the catalog integration that makes the
//! `agent.invoke` palette command appear (or not) for a member:
//!   - READ-SURFACE UNIT: a default-only node lists exactly `{default:"default", runtimes:["default"]}`;
//!     a node with an EXTRA registered runtime lists both (sorted, default among them);
//!   - CAPABILITY-DENY (opaque, §2.1): no `mcp:agent.runtimes:call` → `ToolError::Denied`, no id leaked;
//!   - WORKSPACE-ISOLATION (§2.2): a ws-B principal sees only THIS node's config (registry-derived,
//!     no store read → structurally no cross-ws data), never a ws-A record;
//!   - CATALOG INTEGRATION: a member WITH `mcp:agent.invoke:call` sees the `agent.invoke` command in
//!     `tools.catalog`; one WITHOUT does NOT (absent, no existence leak) — the descriptor name IS the gate.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{list_runtimes, tools_catalog, AgentError, AgentRuntime, Node, RunContext};
use lb_mcp::ToolError;

const RUNTIMES: &str = "mcp:agent.runtimes:call";
const INVOKE: &str = "mcp:agent.invoke:call";
const CATALOG: &str = "mcp:tools.catalog:call";

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

/// A do-nothing external-runtime stand-in: enough to REGISTER a second id in the registry so the read
/// verb has more than `default` to list. Its `run` is never driven here (this file exercises only the
/// read surface); it exists to prove the listing reflects a populated registry, not just the default.
struct StubRuntime(&'static str);

impl AgentRuntime for StubRuntime {
    fn id(&self) -> &str {
        self.0
    }
    fn run<'a>(
        &'a self,
        _node: &'a std::sync::Arc<Node>,
        _ctx: RunContext<'a>,
    ) -> Pin<Box<dyn Future<Output = Result<String, AgentError>> + Send + 'a>> {
        Box::pin(async { Ok(String::new()) })
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn default_only_node_lists_exactly_default() {
    let node = Node::boot().await.expect("node boots");
    let ws = "rt-default";
    let p = principal("user:ada", ws, &[RUNTIMES]);

    let out = list_runtimes(&node, &p, ws).await.expect("authorized list");
    assert_eq!(out["default"], "default");
    let ids: Vec<&str> = out["runtimes"]
        .as_array()
        .expect("runtimes array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec!["default"],
        "a bare node lists exactly the default"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn node_with_an_extra_runtime_lists_both_sorted() {
    let node = Node::boot().await.expect("node boots");
    // Install a registry carrying the default PLUS a registered external stand-in. `default_runtimes`
    // is private, so rebuild the default-only registry the same way boot does, then register the stub.
    let mut registry = default_registry();
    registry.register(Arc::new(StubRuntime("acme-external")));
    node.install_runtimes(registry);

    let ws = "rt-extra";
    let p = principal("user:ada", ws, &[RUNTIMES]);

    let out = list_runtimes(&node, &p, ws).await.expect("authorized list");
    assert_eq!(out["default"], "default", "default id is unchanged");
    let ids: Vec<&str> = out["runtimes"]
        .as_array()
        .expect("runtimes array")
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    // Sorted: `acme-external` before `default`; both present.
    assert_eq!(ids, vec!["acme-external", "default"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_read_cap_the_list_is_denied_opaquely() {
    let node = Node::boot().await.expect("node boots");
    let ws = "rt-deny";
    // Holds an unrelated cap but NOT `mcp:agent.runtimes:call`.
    let p = principal("user:ada", ws, &[INVOKE]);

    let err = list_runtimes(&node, &p, ws)
        .await
        .expect_err("no read cap → denied");
    assert!(
        matches!(err, ToolError::Denied),
        "the deny is opaque (no id leaked), got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ws_b_principal_sees_only_this_nodes_config() {
    // The list is registry-derived (no store read), so there is no ws-A record for a ws-B caller to
    // reach in the first place — but prove the ws-scoped call still succeeds and yields ONLY this
    // node's configured runtimes, identical for any workspace.
    let node = Node::boot().await.expect("node boots");
    let a = principal("user:ada", "ws-a", &[RUNTIMES]);
    let b = principal("user:bob", "ws-b", &[RUNTIMES]);

    let out_a = list_runtimes(&node, &a, "ws-a").await.expect("ws-a list");
    let out_b = list_runtimes(&node, &b, "ws-b").await.expect("ws-b list");
    assert_eq!(
        out_a, out_b,
        "the config is the node's, not the workspace's"
    );
    assert_eq!(out_b["runtimes"], serde_json::json!(["default"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_shows_agent_invoke_only_with_the_invoke_cap() {
    let node = Node::boot().await.expect("node boots");
    let ws = "rt-catalog";

    // WITH the invoke cap → the `agent.invoke` command is present (its name IS the gate).
    let member = principal("user:ada", ws, &[CATALOG, INVOKE]);
    let cat = tools_catalog(&node, &member, ws).await.expect("catalog");
    let cmd = cat
        .tools
        .iter()
        .find(|t| t.name == "agent.invoke")
        .expect("a member with mcp:agent.invoke:call sees the agent command");
    // The descriptor carries the runtime widget hint the palette renders the dropdown from.
    let schema = cmd
        .input_schema
        .as_ref()
        .expect("agent.invoke has a schema");
    assert_eq!(schema["properties"]["runtime"]["x-lb"]["widget"], "runtime");
    assert!(schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "goal"));

    // WITHOUT it → absent (no existence leak), even though the catalog verb itself is held.
    let denied = principal("user:eve", ws, &[CATALOG]);
    let cat2 = tools_catalog(&node, &denied, ws).await.expect("catalog");
    assert!(
        !cat2.tools.iter().any(|t| t.name == "agent.invoke"),
        "a member lacking mcp:agent.invoke:call never sees the agent command"
    );
}

/// Rebuild the boot default-only registry (the in-house `default` over the unconfigured placeholder),
/// mirroring `boot::default_runtimes` — kept here because that constructor is crate-private.
fn default_registry() -> lb_host::RuntimeRegistry {
    lb_host::RuntimeRegistry::with_default(Arc::new(lb_host::UnconfiguredModel))
}
