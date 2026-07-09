//! The headline proof for the **desktop-persistent-store** scope: a `full` boot against a durable
//! `LB_STORE_PATH` keeps user work across a restart. This is the regression for "I restarted the
//! desktop and lost everything" — before persistence, every boot opened a fresh in-memory store.
//!
//! Drives the real path: set an explicit temp `LB_STORE_PATH` (what the shipped app's
//! `store::ensure_store_path` fills in from the per-user data dir), boot `full`, write a record over
//! the loopback gateway, drop the node + gateway, then RE-BOOT at the same path and assert the record
//! is still there. Real SurrealKV on disk, real gateway, real caps — no mocks (rule 9).
//!
//! A single serial test (`LB_STORE_PATH` is process-global): a distinct temp dir per run, cleaned up
//! at the end. Registers a source the boot seeders do NOT create (`my-source`), so a survivor proves
//! USER data persisted — not just the idempotently re-seeded `demo-buildings`.

#![cfg(feature = "full")]

use std::net::SocketAddr;
use std::sync::Arc;

use lazybones_shell::full::boot_full;
use lb_host::Node;
use serde_json::{json, Value};

async fn login_token(client: &reqwest::Client, base: &str) -> String {
    client
        .post(format!("{base}/login"))
        .json(&json!({"user":"user:ada","workspace":"acme"}))
        .send()
        .await
        .expect("login request")
        .error_for_status()
        .expect("login 200")
        .json::<Value>()
        .await
        .expect("login json")["token"]
        .as_str()
        .expect("token")
        .to_string()
}

async fn source_names(client: &reqwest::Client, base: &str, token: &str) -> Vec<String> {
    let listed: Value = client
        .post(format!("{base}/mcp/call"))
        .bearer_auth(token)
        .json(&json!({"tool":"datasource.list","args":{}}))
        .send()
        .await
        .expect("datasource.list")
        .error_for_status()
        .expect("list 200")
        .json()
        .await
        .expect("list json");
    listed
        .get("datasources")
        .and_then(|s| s.as_array())
        .or_else(|| listed.as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|d| d["name"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_data_survives_a_full_restart_on_a_persistent_store() {
    // A durable store dir for this run — the same knob `store::ensure_store_path` fills from the
    // per-user data dir in the shipped app. SurrealKV creates the dir itself.
    let store_dir = std::env::temp_dir().join(format!("lb-persist-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&store_dir);
    std::env::set_var("LB_STORE_PATH", &store_dir);
    let client = reqwest::Client::new();

    // ---- BOOT #1: write a user record (a datasource the seeders do NOT create). ----
    {
        let node = Arc::new(Node::boot().await.expect("node boots (run 1)"));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let (_gw, bound) = boot_full(node, "acme", addr)
            .await
            .expect("boot_full run 1");
        let base = format!("http://{bound}");
        let token = login_token(&client, &base).await;

        client
            .post(format!("{base}/mcp/call"))
            .bearer_auth(&token)
            .json(&json!({"tool":"datasource.add","args":{
                "name":"my-source","kind":"sqlite","endpoint":"127.0.0.1:0",
                "dsn":"/tmp/whatever-my-source.db","ts":100
            }}))
            .send()
            .await
            .expect("datasource.add my-source")
            .error_for_status()
            .expect("add 200 (run 1)");

        let names = source_names(&client, &base, &token).await;
        assert!(
            names.contains(&"my-source".to_string()),
            "the user source is present in run 1: {names:?}"
        );
        // Node + gateway task drop at end of scope — the store handle closes with them.
    }

    // A moment for the SurrealKV handle to fully release before re-opening the same path.
    tokio::task::yield_now().await;

    // ---- BOOT #2: re-open the SAME store path. The user record must still be there. ----
    {
        let node = Arc::new(Node::boot().await.expect("node boots (run 2)"));
        let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
        let (_gw, bound) = boot_full(node, "acme", addr)
            .await
            .expect("boot_full run 2");
        let base = format!("http://{bound}");
        let token = login_token(&client, &base).await;

        let names = source_names(&client, &base, &token).await;
        assert!(
            names.contains(&"my-source".to_string()),
            "THE REGRESSION: user work must survive a restart on a persistent store — \
             `my-source` was gone after re-boot: {names:?}"
        );
    }

    std::env::remove_var("LB_STORE_PATH");
    let _ = std::fs::remove_dir_all(&store_dir);
}
