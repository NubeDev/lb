//! The unified extension lifecycle surface at the host layer (lifecycle-management scope): `ext.list`
//! (union both tiers), `ext.enable`/`disable` (durable intent), `ext.uninstall` (idempotent,
//! workspace-first), and the boot `reconcile` honoring `enabled`. Mandatory capability-deny +
//! two-workspace isolation, plus the slice's load-bearing case: a **disabled** extension is NOT in
//! the boot reconcile start-plan (it must not silently return after a restart).

use lb_assets::{record_install, Install, Tier};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{ext_disable, ext_enable, ext_list, ext_uninstall, reconcile, Node};

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

const ALL: &[&str] = &[
    "mcp:ext.list:call",
    "mcp:ext.disable:call",
    "mcp:ext.uninstall:call",
];

async fn seed(node: &Node, ws: &str, ext: &str, tier: Tier) {
    let install = Install::new(ext, "v1", vec![], 1).with_tier(tier);
    record_install(&node.store, ws, &install).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn denies_each_lifecycle_verb_without_its_cap() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    seed(&node, ws, "hello", Tier::Wasm).await;
    let none = principal("user:mallory", ws, &[]);

    assert!(ext_list(&node, &none, ws).await.is_err());
    assert!(ext_disable(&node, &none, ws, "hello", 2).await.is_err());
    assert!(ext_enable(&node, &none, ws, "hello", 2).await.is_err());
    assert!(ext_uninstall(&node, &none, ws, "hello", 2).await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_list_or_uninstall_ws_a_extensions() {
    let node = Node::boot().await.unwrap();
    seed(&node, "acme", "hello", Tier::Wasm).await;
    let admin_b = principal("user:carol", "globex", ALL);

    // ws-B lists its own (empty) namespace — never ws-A's install.
    assert!(ext_list(&node, &admin_b, "globex")
        .await
        .unwrap()
        .is_empty());
    // ws-B uninstalling a ws-A id touches nothing in ws-A (it operates on globex's namespace).
    ext_uninstall(&node, &admin_b, "globex", "hello", 2)
        .await
        .unwrap();
    let admin_a = principal("user:alice", "acme", ALL);
    assert_eq!(
        ext_list(&node, &admin_a, "acme").await.unwrap().len(),
        1,
        "ws-A's extension survived a ws-B uninstall"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_unions_both_tiers_and_reflects_enable_disable_uninstall() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let admin = principal("user:alice", ws, ALL);
    seed(&node, ws, "hello", Tier::Wasm).await;
    seed(&node, ws, "echo-sidecar", Tier::Native).await;

    // list unions both tiers.
    let rows = ext_list(&node, &admin, ws).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r.ext == "hello" && r.tier == "wasm"));
    assert!(rows
        .iter()
        .any(|r| r.ext == "echo-sidecar" && r.tier == "native"));

    // disable → the row reflects disabled (not enabled); enable round-trips back.
    ext_disable(&node, &admin, ws, "hello", 2).await.unwrap();
    let rows = ext_list(&node, &admin, ws).await.unwrap();
    let hello = one(&rows, "hello");
    assert!(!hello.enabled && hello.health == "disabled");
    ext_enable(&node, &admin, ws, "hello", 3).await.unwrap();
    let rows = ext_list(&node, &admin, ws).await.unwrap();
    assert!(one(&rows, "hello").enabled);

    // uninstall → the row disappears; re-uninstall is a no-op success (idempotent).
    ext_uninstall(&node, &admin, ws, "hello", 4).await.unwrap();
    assert!(!ext_list(&node, &admin, ws)
        .await
        .unwrap()
        .iter()
        .any(|r| r.ext == "hello"));
    ext_uninstall(&node, &admin, ws, "hello", 5).await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn boot_reconcile_honors_disable_intent() {
    let node = Node::boot().await.unwrap();
    let ws = "acme";
    let admin = principal("user:alice", ws, ALL);
    seed(&node, ws, "hello", Tier::Wasm).await; // enabled
    seed(&node, ws, "echo-sidecar", Tier::Native).await; // enabled
    ext_disable(&node, &admin, ws, "echo-sidecar", 2)
        .await
        .unwrap();

    let plan = reconcile(&node, ws).await.unwrap();
    let hello = plan.actions.iter().find(|a| a.ext == "hello").unwrap();
    let echo = plan
        .actions
        .iter()
        .find(|a| a.ext == "echo-sidecar")
        .unwrap();
    assert!(hello.start, "enabled extension is planned to start on boot");
    assert!(
        !echo.start && echo.reason == "disabled",
        "a DISABLED extension must NOT auto-start on boot"
    );
}

fn one<'a>(rows: &'a [lb_host::ExtRow], ext: &str) -> &'a lb_host::ExtRow {
    rows.iter().find(|r| r.ext == ext).expect("row present")
}
