//! The `node` binary entry point. Selects roles by config (S1: solo) and drives the host.
//!
//! In S1 it boots a solo node, loads the `hello` extension, and calls `hello.echo` once to
//! prove the spine is live end to end. Real role selection + config + the SSE gateway arrive
//! at S3; the UI at S2. Kept to one verb (FILE-LAYOUT): everything substantive is in `lb-host`.

use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{load_enabled, load_extension, Node};

mod federation;
mod github;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Boot the spine (solo node). `Arc` so the env-gated background roles (webhook + workflow driver)
    // can share it with the demo path below.
    let node = Arc::new(Node::boot().await?);

    // Locate the built hello component. Override with HELLO_WASM; default to the cargo target.
    let wasm_path = std::env::var("HELLO_WASM")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm")
        });
    let manifest = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../extensions/hello/extension.toml"),
    )?;
    let wasm = std::fs::read(&wasm_path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", wasm_path.display()))?;

    let loaded = load_extension(&node, &manifest, &wasm, &[]).await?;

    // BOOT BRING-UP (lifecycle-management): re-load every previously-published-and-enabled wasm
    // extension for the configured workspace from the durable cache, so an upload survives a restart
    // (the durable Install record + the digest-keyed verified cache are the source of truth). A no-op
    // on a fresh store. The workspace comes from `LB_WORKSPACE` (the dev launch sets it; default "acme").
    let ws = std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());
    match load_enabled(&node, &ws).await {
        Ok(loaded_exts) => {
            for e in loaded_exts.iter().filter(|e| e.loaded) {
                println!(
                    "boot-loaded extension: {}@{} ({})",
                    e.ext, e.version, e.reason
                );
            }
        }
        Err(e) => eprintln!("boot extension load for ws={ws} failed: {e}"),
    }

    // ROLE SELECTION (config, §3.1): mount the github-workflow ingress + background driver if the
    // environment configures them. A no-op otherwise — the binary stays the solo demo below.
    github::mount(node.clone()).await;
    // datasources role (federation native sidecar), env-gated by LB_FEDERATION_ENDPOINTS.
    federation::mount(node.clone()).await;
    println!(
        "loaded hello: tools={:?} granted_caps={:?}",
        loaded.tools, loaded.granted_caps
    );

    // Mint a member token for workspace "acme" that may call hello.echo.
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:demo".into(),
        ws: "acme".into(),
        role: Role::Member,
        caps: vec!["mcp:hello.echo:call".into()],
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    let principal = lb_auth::verify(&key, &token, 1).expect("freshly minted token verifies");

    // Call the tool through the MCP pipeline (resolve → authorize → dispatch).
    let out = lb_mcp::call(
        &node.registry,
        &node.bus,
        &principal,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await?;
    println!("hello.echo -> {out}");

    // ROLE SELECTION (config, not a code branch in core crates): if `LB_GATEWAY_ADDR` is set,
    // mount the SSE/HTTP gateway so a browser can reach a real node (S3). Otherwise the binary
    // is the solo demo above. This is the thin wiring layer §3.1 permits to be role-aware.
    if let Ok(addr) = std::env::var("LB_GATEWAY_ADDR") {
        let addr: std::net::SocketAddr = addr
            .parse()
            .map_err(|e| anyhow::anyhow!("bad LB_GATEWAY_ADDR: {e}"))?;
        // The gateway fronts THIS node. Do not call `Gateway::boot()` here: that would open a second
        // embedded store handle, and with `LB_STORE_PATH` set both handles would point at the same
        // SurrealKV directory.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let gw = lb_role_gateway::Gateway::new(node.clone(), SigningKey::generate(), now);
        println!("gateway: serving on http://{addr}");
        lb_role_gateway::serve(gw, addr).await?;
    }

    Ok(())
}
