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
    let mut node = Node::boot().await?;

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

    let loaded = load_extension(&mut node, &manifest, &wasm, &[]).await?;
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
        &principal,
        "acme",
        "hello.echo",
        r#"{"msg":"hi"}"#,
    )
    .await?;
    println!("hello.echo -> {out}");

    Ok(())
}
