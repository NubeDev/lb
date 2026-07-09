//! The headline proof for the **desktop-federation-bundle** scope: the standalone `full` boot
//! auto-installs the bundled federation sidecar and pre-registers the shipped sqlite demo, so a
//! packaged binary can register AND query a datasource out of the box — closing the "register but
//! test/query is denied" gap that motivated the scope.
//!
//! This drives the EXACT path the double-clicked `.exe` runs: `boot_full` → `mount_federation` →
//! `install_federation`, then a real HTTP client over the loopback gateway hits `datasource.test`
//! (the call that used to return an opaque "denied" with no sidecar) and `federation.query`. No
//! mocks (rule 9): the REAL supervisor spawns the REAL sidecar; the store/gateway/caps are real; the
//! external db is a REAL on-disk sqlite file (testing §0's one sanctioned external boundary).
//!
//! One test, run serially (`worker_threads = 1`, a single `#[tokio::test]` in this bin), because
//! `mount_federation` reads `LB_FEDERATION_DIR` from the PROCESS env — a second concurrent boot in
//! the same process would race it. It bundles the mandatory categories (deny + workspace-isolation)
//! alongside the green e2e, mirroring `federation_sqlite_test.rs`.

#![cfg(feature = "full")]

use std::net::SocketAddr;
use std::process::Command;
use std::sync::Arc;

use lazybones_shell::full::boot_full;
use lb_host::Node;
use serde_json::{json, Value};

/// Build the sidecar with DEFAULT features (sqlite only — no postgres/TLS toolchain) and return the
/// dir holding it. Sqlite is not feature-gated, so this needs nothing external — a build failure is a
/// FAIL, not a skip (the desktop bundle depends on this exact binary existing).
fn build_sidecar_dir() -> std::path::PathBuf {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // ui/src-tauri → repo root → rust/ (the workspace the `federation` bin lives in).
    let rust_dir = manifest_dir.join("../../rust");
    let target = rust_dir.join("target/debug");
    let status = Command::new("cargo")
        .args(["build", "-p", "federation"])
        .current_dir(&rust_dir)
        .status()
        .expect("cargo build -p federation runs");
    assert!(
        status.success() && target.join("federation").exists(),
        "the default-features (sqlite) federation sidecar builds for the desktop bundle"
    );
    target.canonicalize().expect("canonicalize target/debug")
}

/// Lay out a bundle dir exactly like the packaging: the `federation` binary + a `demo-buildings.db`
/// beside it. We hard-link (fall back to copy) the built binary so `mount_federation` resolves it via
/// `LB_FEDERATION_DIR`, and seed a small real demo db under the name the desktop pre-registers.
fn stage_bundle(sidecar_dir: &std::path::Path) -> std::path::PathBuf {
    let bundle = std::env::temp_dir().join(format!("lb-desktop-fed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&bundle);
    std::fs::create_dir_all(&bundle).expect("create bundle dir");

    let src = sidecar_dir.join("federation");
    let dst = bundle.join("federation");
    // Hard-link is cheap; a cross-device temp dir falls back to a copy.
    if std::fs::hard_link(&src, &dst).is_err() {
        std::fs::copy(&src, &dst).expect("copy sidecar into bundle");
    }

    let db = bundle.join("demo-buildings.db");
    let conn = rusqlite::Connection::open(&db).expect("open demo db");
    conn.execute_batch(
        "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL);
         INSERT INTO site VALUES ('site-001','Northside Factory'),('site-002','City Tower');",
    )
    .expect("seed demo db");
    bundle
}

async fn login(client: &reqwest::Client, base: &str, user: &str, ws: &str) -> reqwest::Response {
    client
        .post(format!("{base}/login"))
        .json(&json!({"user": user, "workspace": ws}))
        .send()
        .await
        .expect("login request")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_boot_bundles_federation_and_queries_sqlite_demo() {
    let sidecar_dir = build_sidecar_dir();
    let bundle = stage_bundle(&sidecar_dir);
    // The desktop resolves the sidecar + demo db beside its own exe; the test points that resolution
    // at the staged bundle. Set BEFORE boot (mount_federation reads it during boot_full).
    std::env::set_var("LB_FEDERATION_DIR", &bundle);

    let node = Arc::new(Node::boot().await.expect("node boots"));
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_gw, bound) = boot_full(node, "acme", addr)
        .await
        .expect("boot_full binds a loopback gateway");
    let base = format!("http://{bound}");
    let client = reqwest::Client::new();

    // --- Login as the seeded admin over the loopback gateway. ---
    let token = login(&client, &base, "user:ada", "acme")
        .await
        .error_for_status()
        .expect("login 200")
        .json::<Value>()
        .await
        .expect("login json")["token"]
        .as_str()
        .expect("token")
        .to_string();

    // --- The demo source is pre-registered (the seed step). It shows up on datasource.list. ---
    let listed: Value = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"datasource.list","args":{}}))
        .send()
        .await
        .expect("datasource.list")
        .error_for_status()
        .expect("list 200")
        .json()
        .await
        .expect("list json");
    // The tool bridge wraps the rows as `{"datasources":[…]}`; be tolerant of a bare array too.
    let rows = listed
        .get("datasources")
        .and_then(|s| s.as_array())
        .or_else(|| listed.as_array())
        .unwrap_or_else(|| panic!("datasource.list shape unexpected: {listed}"));
    let names: Vec<&str> = rows.iter().filter_map(|d| d["name"].as_str()).collect();
    assert!(
        names.contains(&"demo-buildings"),
        "the bundled demo source is pre-registered: {names:?}"
    );
    assert!(
        !listed.to_string().contains("demo-buildings.db"),
        "the path DSN is mediated into secrets, never listed (§6.7)"
    );

    // --- THE REGRESSION: datasource.test used to return an opaque 'denied' with no sidecar. With
    // the bundled sidecar installed + the sqlite endpoint approved, it now probes GREEN. ---
    let probe: Value = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"datasource.test","args":{"source":"demo-buildings","ts":10}}))
        .send()
        .await
        .expect("datasource.test")
        .error_for_status()
        .expect("test 200 (was: denied)")
        .json()
        .await
        .expect("test json");
    assert_eq!(probe["ok"], true, "the demo source probes green: {probe}");

    // --- And a real query returns the seeded rows (queryable, not just present). ---
    let q: Value = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"federation.query","args":{
            "source":"demo-buildings","sql":"SELECT id, name FROM site ORDER BY id","ts":11
        }}))
        .send()
        .await
        .expect("federation.query")
        .error_for_status()
        .expect("query 200")
        .json()
        .await
        .expect("query json");
    assert_eq!(
        q["rows"].as_array().map(|r| r.len()),
        Some(2),
        "the two seeded sites come back: {q}"
    );

    // --- CAPABILITY-DENY (mandatory): an endpoint NOT in the approved grant is refused pre-connect
    // even with the sidecar present. Register a postgres source at a real host:port and probe it —
    // the desktop grant approves ONLY 127.0.0.1:0, so enforce_endpoint denies (the headline wall). ---
    client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"datasource.add","args":{
            "name":"remote-pg","kind":"postgres","endpoint":"db.example:5432",
            "dsn":"postgres://u:p@db.example:5432/x","ts":12
        }}))
        .send()
        .await
        .expect("datasource.add remote-pg")
        .error_for_status()
        .expect("add 200 (registration is allowed; the connect is what's walled)");
    let denied = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"datasource.test","args":{"source":"remote-pg","ts":13}}))
        .send()
        .await
        .expect("datasource.test remote-pg");
    assert!(
        !denied.status().is_success(),
        "an unapproved endpoint is refused pre-connect (got {}), the deny wall holds with the sidecar present",
        denied.status()
    );

    // --- WORKSPACE ISOLATION (mandatory): a source registered in `acme` ONLY is not resolvable from
    // a second workspace. (Each boot independently seeds its own `demo-buildings`, so isolation must
    // be proven on a source that is NOT re-seeded — an `acme`-only record.) Register `acme-only` in
    // `acme`, boot a second node for ws `other`, and confirm `other` cannot resolve it: the datasource
    // record is a workspace-scoped key, so the lookup misses (BadInput/not-found), never a leak. ---
    client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(&token)
        .json(&json!({"tool":"datasource.add","args":{
            "name":"acme-only","kind":"sqlite","endpoint":"127.0.0.1:0",
            "dsn": bundle.join("demo-buildings.db").to_string_lossy(), "ts":14
        }}))
        .send()
        .await
        .expect("datasource.add acme-only")
        .error_for_status()
        .expect("add 200 in acme");

    let node_b = Arc::new(Node::boot().await.expect("node-b boots"));
    let addr_b: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (_gw_b, bound_b) = boot_full(node_b, "other", addr_b)
        .await
        .expect("boot_full binds for ws other");
    let base_b = format!("http://{bound_b}");
    let token_b = login(&client, &base_b, "user:ada", "other")
        .await
        .error_for_status()
        .expect("login 200 in ws other")
        .json::<Value>()
        .await
        .expect("login json")["token"]
        .as_str()
        .expect("token")
        .to_string();
    let iso = client
        .post(format!("{base_b}/mcp/call"))
        .bearer_auth(&token_b)
        .json(&json!({"tool":"federation.query","args":{
            "source":"acme-only","sql":"SELECT 1","ts":15
        }}))
        .send()
        .await
        .expect("cross-ws query attempt");
    assert!(
        !iso.status().is_success(),
        "ws `other` cannot resolve `acme`'s acme-only source (got {}), the workspace wall holds",
        iso.status()
    );

    std::env::remove_var("LB_FEDERATION_DIR");
    let _ = std::fs::remove_dir_all(&bundle);
}
