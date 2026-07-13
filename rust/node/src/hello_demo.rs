//! The S1 `hello` demo bring-up (moved verbatim from `main.rs`) — load the `hello` wasm extension and
//! call `hello.echo` once to prove the spine is live end to end. Gated by [`BootConfig::hello_demo`]:
//! the `node` binary runs it (today's behaviour); an embedder wants it OFF (no demo extension).

use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{load_extension, Node};

/// Load the built `hello` component (override with `HELLO_WASM`; default the cargo target), install it,
/// and call `hello.echo` once through the MCP pipeline. Best-effort proof-of-spine; returns an error
/// only if the wasm/manifest cannot be read or the load fails (matching today's `main.rs` `?`).
pub async fn run(node: &Arc<Node>) -> anyhow::Result<()> {
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

    let loaded = load_extension(node, &manifest, &wasm, &[]).await?;
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
        constraint: None,
        run_id: None,
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
    Ok(())
}
