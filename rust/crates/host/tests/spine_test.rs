//! The S1 EXIT-GATE test (STAGES.md S1, mcp scope): a tool call routed through MCP succeeds
//! WITH the grant and is refused WITHOUT it; a second workspace cannot reach the first.
//!
//! This exercises the whole spine end to end with the REAL wasm component (testing §3: real,
//! not mocked): host boot → load → caps → WIT → WASM → back. The three tests here ARE the
//! mandatory capability-deny and workspace-isolation categories for S1.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{load_extension, Node};
use lb_mcp::{call, ToolError};

const MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");

/// Read the built hello component. The wasm guest is built separately (it targets
/// wasm32-wasip2); CI builds it before `cargo test`. If it's missing, fail loudly with how to
/// build it rather than silently skipping (testing: no silent gaps).
fn hello_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing hello component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/hello && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}

/// Factory: a verified principal in `ws` holding `caps` (testing §3 fixtures).
fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Boot a node and load hello into it.
async fn node_with_hello() -> Node {
    let node = Node::boot().await.expect("node boots");
    load_extension(&node, MANIFEST, &hello_wasm(), &[])
        .await
        .expect("hello loads");
    node
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_succeeds_with_the_grant() {
    let node = node_with_hello().await;
    let p = principal("acme", &["mcp:hello.echo:call"]);

    let out = call(
        &node.registry,
        &node.bus,
        &p,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect("granted call succeeds");

    let value: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(value["echo"], "hi");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn echo_is_refused_without_the_grant() {
    // MANDATORY capability-deny (testing §2.1): same call, no grant → Denied.
    let node = node_with_hello().await;
    let p = principal("acme", &[]); // no caps

    let err = call(
        &node.registry,
        &node.bus,
        &p,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect_err("ungranted call is refused");
    assert_eq!(err, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denied_call_does_not_reveal_tool_existence() {
    // A nonexistent tool, called without authorization, returns the SAME opaque Denied as a
    // real-but-ungranted tool — so a caller cannot probe which tools exist (mcp scope).
    let node = node_with_hello().await;
    let p = principal("acme", &[]);

    let real = call(&node.registry, &node.bus, &p, "acme", "hello.echo", "{}")
        .await
        .unwrap_err();
    let fake = call(&node.registry, &node.bus, &p, "acme", "hello.nope", "{}")
        .await
        .unwrap_err();
    assert_eq!(real, ToolError::Denied);
    assert_eq!(fake, ToolError::Denied);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn second_workspace_cannot_call_into_the_first() {
    // MANDATORY workspace-isolation (testing §2.2): a principal in workspace "other", even
    // holding the matching capability, is denied because the call targets workspace "acme"
    // and the isolation gate fires first (auth-caps scope).
    let node = node_with_hello().await;
    let p = principal("other", &["mcp:hello.echo:call"]);

    let err = call(
        &node.registry,
        &node.bus,
        &p,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect_err("cross-workspace call is refused");
    assert_eq!(err, ToolError::Denied);
}
