//! MANDATORY hot-reload test (testing §2.4): swap an extension version LIVE and assert no
//! durable state is lost — the stateless-extension guarantee (§3.4).
//!
//! The proof has three parts, end to end through the real wasm:
//!   1. with v1 loaded, post channel messages (durable STATE in the store) and call the tool;
//!   2. `reload_extension` to v2 (different wasm, bumped version) while the node keeps running;
//!   3. assert: the channel history is INTACT (state survived the swap), AND the tool now
//!      answers with v2's shape (`"v": 2`) — proving the instance was really replaced, not a
//!      no-op. Durable state lives in the store/bus, never in the instance, so the swap is safe.
//!
//! Multi-thread flavor required (boots a Zenoh peer; debugging/bus/zenoh-needs-multi-thread-runtime.md).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{history, load_extension, post, reload_extension, Node};
use lb_inbox::Item;
use lb_mcp::call;

const MANIFEST_V1: &str = include_str!("../../../extensions/hello/extension.toml");
const MANIFEST_V2: &str = include_str!("../../../extensions/hello-v2/extension.toml");

fn wasm(rel: &str) -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  \
             (cd rust/extensions/<ext> && cargo build --target wasm32-wasip2 --release)",
            path.display()
        )
    })
}

fn hello_v1() -> Vec<u8> {
    wasm("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm")
}
fn hello_v2() -> Vec<u8> {
    wasm("../../extensions/hello-v2/target/wasm32-wasip2/release/hello_v2_ext.wasm")
}

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

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn swapping_an_extension_version_keeps_durable_state() {
    let ws = "hot-reload-ws";
    let node = Node::boot().await.expect("node boots");

    // --- v1 online ---
    load_extension(&node, MANIFEST_V1, &hello_v1(), &[])
        .await
        .expect("hello v1 loads");

    let p = principal(
        ws,
        &[
            "bus:chan/general:pub",
            "bus:chan/general:sub",
            "mcp:hello.echo:call",
        ],
    );

    // Durable STATE: three posted messages, persisted to the store.
    for (i, body) in ["a", "b", "c"].iter().enumerate() {
        post(
            &node,
            &p,
            ws,
            "general",
            Item::new(format!("m{i}"), "general", "user:p", *body, i as u64),
        )
        .await
        .expect("post");
    }

    // v1 answers (no version field).
    let v1_out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect("v1 echo");
    let v1: serde_json::Value = serde_json::from_str(&v1_out).unwrap();
    assert_eq!(v1["echo"], "hi");
    assert!(v1.get("v").is_none(), "v1 output has no version field");

    // --- LIVE SWAP to v2 ---
    let loaded = reload_extension(&node, MANIFEST_V2, &hello_v2(), &[])
        .await
        .expect("hot-reload to v2 succeeds");
    assert!(loaded.tools.contains(&"echo".to_string()));

    // 1. Durable state INTACT — the channel history survived the swap untouched.
    let after = history(&node.store, &p, ws, "general")
        .await
        .expect("history after reload");
    let bodies: Vec<&str> = after.iter().map(|i| i.body.as_str()).collect();
    assert_eq!(
        bodies,
        ["a", "b", "c"],
        "durable channel history must survive a hot-reload"
    );

    // 2. The swap really took effect — v2 answers with its new shape.
    let v2_out = call(
        &node.registry,
        &node.bus,
        &p,
        ws,
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await
    .expect("v2 echo");
    let v2: serde_json::Value = serde_json::from_str(&v2_out).unwrap();
    assert_eq!(v2["echo"], "hi");
    assert_eq!(v2["v"], 2, "after reload the v2 instance must answer");

    // 3. The channel still works after the swap — post + read another message.
    post(
        &node,
        &p,
        ws,
        "general",
        Item::new("m3", "general", "user:p", "d", 3),
    )
    .await
    .expect("post after reload");
    let final_view = history(&node.store, &p, ws, "general").await.unwrap();
    assert_eq!(final_view.len(), 4, "channel keeps working post-swap");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn reloading_an_uninstalled_extension_is_rejected() {
    // A reload swaps an EXISTING id; installing a brand-new id is load_extension's job. This
    // guards the swap semantics (no accidental silent install through the reload path).
    let node = Node::boot().await.expect("node boots");
    let err = reload_extension(&node, MANIFEST_V2, &hello_v2(), &[])
        .await
        .expect_err("reload of a never-installed extension is refused");
    // It's a manifest-class error carrying the id; just assert it failed loudly.
    assert!(format!("{err}").contains("hello"));
}
