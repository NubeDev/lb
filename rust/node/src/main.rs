//! The `node` binary entry point. Selects roles by config (S1: solo) and drives the host.
//!
//! In S1 it boots a solo node, loads the `hello` extension, and calls `hello.echo` once to
//! prove the spine is live end to end. Real role selection + config + the SSE gateway arrive
//! at S3; the UI at S2. Kept to one verb (FILE-LAYOUT): everything substantive is in `lb-host`.

use std::path::PathBuf;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{load_extension, Node};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Boot the spine (solo node).
    let node = Node::boot().await?;

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
        let ws = std::env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());
        let addr: std::net::SocketAddr = addr
            .parse()
            .map_err(|e| anyhow::anyhow!("bad LB_GATEWAY_ADDR: {e}"))?;
        let gw = lb_role_gateway::Gateway::boot(&ws)
            .await
            .map_err(|e| anyhow::anyhow!("gateway boot: {e}"))?;
        println!("gateway: serving workspace '{ws}' on http://{addr}");
        lb_role_gateway::serve(gw, addr).await?;
    }

    Ok(())
}
