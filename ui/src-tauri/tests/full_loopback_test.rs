//! The headline proof for the `full` feature: a non-windowed boot of the standalone backend
//! answers a real HTTP client — login returns a signed token, and that token drives a real
//! `POST /mcp/call` (the `tools.catalog` host verb). No display, no webview, no Tauri window
//! — just the node + the in-process gateway + real HTTP, which is the whole point of the
//! `full` mode (the packaged binary works as a 100% standalone node).
//!
//! Binds `127.0.0.1:0` for a collision-free port (the reason `boot_full` returns the bound
//! addr). Real store, real bus, real gateway, real caps — rule 9 (no mocks).

#![cfg(feature = "full")]

use std::net::SocketAddr;
use std::sync::Arc;

use lazybones_shell::full::boot_full;
use lb_host::Node;
use serde_json::Value;

/// Drive a login over the loopback gateway and then a real MCP call with the returned token.
/// The minted token IS the wall (§7): `mcp/call` derives principal + workspace from it, not
/// the request — so a token minted for `user:ada` / `acme` reaching `tools.catalog` is the
/// end-to-end proof that the boot seed + gateway + capability pipeline all stand up together.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_then_mcp_call_works_over_the_loopback_gateway() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_gw, bound) = boot_full(node, "acme", addr)
        .await
        .expect("boot_full binds a loopback gateway");
    let base = format!("http://{bound}");

    // Login as the seeded dev user. The boot seeder made `user:ada` a workspace-admin member
    // of `acme`, so this must mint a real signed token (the membership gate passes).
    let client = reqwest::Client::new();
    let login: Value = client
        .post(format!("{base}/login"))
        .json(&serde_json::json!({"user":"user:ada","workspace":"acme"}))
        .send()
        .await
        .expect("login request")
        .error_for_status()
        .expect("login 200")
        .json()
        .await
        .expect("login json");
    let token = login["token"]
        .as_str()
        .expect("login reply carries a token")
        .to_string();
    assert!(!token.is_empty(), "the token is non-empty");
    assert_eq!(login["workspace"].as_str(), Some("acme"));
    assert_eq!(login["principal"].as_str(), Some("user:ada"));

    // The minted token drives a real MCP call through the gateway's capability pipeline.
    // `tools.catalog` is a granted read; a non-empty catalog proves the seeded principal
    // resolves caps AND the host tool dispatcher answers over the loopback HTTP transport.
    let catalog: Value = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&serde_json::json!({"tool":"tools.catalog","args":{}}))
        .send()
        .await
        .expect("mcp/call request")
        .error_for_status()
        .expect("mcp/call 200")
        .json()
        .await
        .expect("catalog json");
    // The catalog is non-empty (the node boots with host tools + seeded agent definitions).
    // Shape-agnostic: just assert the call returned an array with at least one entry.
    let arr = catalog
        .as_array()
        .or_else(|| catalog.get("tools").and_then(|t| t.as_array()))
        .expect("catalog is an array (or {tools:[]})");
    assert!(!arr.is_empty(), "the catalog is non-empty");
}

/// Regression (missing `ensure_builtin_authz_roles` in `boot_full`): the seeded `user:ada` is
/// granted `role:workspace-admin`, but if the built-in role RECORDS are never seeded the grant
/// resolves to ZERO caps — the user logs in but is "missing access to everything" (every admin
/// surface 403s). This drives a real ADMIN-ONLY route (`GET /admin/roles`, gated
/// `mcp:roles.list:call`) with the seeded token: a 200 proves the workspace-admin cap bundle
/// actually reached the token. Before the fix this was a 403; `tools.catalog` (a viewer read)
/// passed regardless, so it never caught the gap.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn seeded_user_resolves_to_a_real_workspace_admin() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_gw, bound) = boot_full(node, "acme", addr)
        .await
        .expect("boot_full binds");
    let base = format!("http://{bound}");

    let client = reqwest::Client::new();
    let login: Value = client
        .post(format!("{base}/login"))
        .json(&serde_json::json!({"user":"user:ada","workspace":"acme"}))
        .send()
        .await
        .expect("login request")
        .error_for_status()
        .expect("login 200")
        .json()
        .await
        .expect("login json");
    let token = login["token"].as_str().expect("token").to_string();

    // `GET /admin/roles` is admin-gated (`mcp:roles.list:call`, in the admin-only bundle). A
    // workspace-admin gets 200; a capless user gets 403. This is the wall the fix restores.
    let resp = client
        .get(format!("{base}/admin/roles"))
        .bearer_auth(&token)
        .send()
        .await
        .expect("admin/roles request");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "seeded user:ada must reach the admin roles route (workspace-admin caps resolved)"
    );
    // And the built-in roles are actually seeded (the records the grant resolves against).
    let roles: Value = resp.json().await.expect("roles json");
    let names: Vec<&str> = roles
        .as_array()
        .expect("roles is an array")
        .iter()
        .filter_map(|r| r["name"].as_str())
        .collect();
    assert!(
        names.contains(&"workspace-admin"),
        "the workspace-admin role record must be seeded (got {names:?})"
    );
}

/// A login for a user NOT seeded into the workspace is refused — the membership gate holds
/// over the loopback gateway exactly as it does against `make dev`. This is the mandatory
/// capability/deny contract (testing-scope §capability-deny) applied to the standalone boot:
/// the seeded wall is not bypassed by the desktop transport.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_refuses_an_unseeded_user() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_gw, bound) = boot_full(node, "acme", addr)
        .await
        .expect("boot_full binds");
    let base = format!("http://{bound}");

    let client = reqwest::Client::new();
    // `user:stranger` is NOT a member of `acme` (only `user:ada` was seeded). The workspace
    // already has a member, so the bootstrap-admin path does NOT fire; the gate refuses.
    let status = client
        .post(format!("{base}/login"))
        .json(&serde_json::json!({"user":"user:stranger","workspace":"acme"}))
        .send()
        .await
        .expect("login request")
        .status();
    assert!(
        status == reqwest::StatusCode::FORBIDDEN,
        "an unseeded user is refused (got {status}), not silently minted a token"
    );
}
