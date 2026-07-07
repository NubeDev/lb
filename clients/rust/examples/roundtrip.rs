//! roundtrip.rs — login → write a Sample → read it back, against a real
//! `make cloud` node.
//!
//! Run with:
//!   make cloud                  # in one terminal — boots 127.0.0.1:8080
//!   cd clients/rust && \
//!     cargo run --example roundtrip -- \
//!       --url http://127.0.0.1:8080 --user ada --workspace acme
//!
//! Or, with an API key (no `/login`):
//!   cargo run --example roundtrip -- \
//!     --url http://127.0.0.1:8080 --key lbk_acme.k7f3a.ABCDEF23

use lb_client::{call_mcp, latest_sample, write_samples, Client};
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let url = env::var("LB_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".into());
    let key = env::var("LB_KEY"); // API key, optional
    let user = env::var("LB_USER").unwrap_or_else(|_| "ada".into());
    let ws = env::var("LB_WORKSPACE").unwrap_or_else(|_| "acme".into());

    let client = Client::new(&url, "placeholder"); // bearer set below
    let client = match key {
        Ok(k) => client.with_bearer(&k),
        Err(_) => {
            let (c, reply) = client.login(&user, &ws).await?;
            println!("logged in as {} in {}", reply.principal, reply.workspace);
            c
        }
    };

    // 1. Push one Sample. `producer` is host-forced to the principal, so omit it.
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_millis() as u64;
    let sample = json!({
        "series": "demo.cpu_temp",
        "ts": now_ms,
        "seq": 1,
        "payload": 61.4,
        "labels": { "host": "pi-7" },
    });
    let written = write_samples(&client, vec![sample]).await?;
    println!("accepted={} committed={}", written.accepted, written.committed);

    // 2. Read the newest value back — the round-trip.
    let latest = latest_sample(&client, "demo.cpu_temp").await?;
    println!("latest sample: {}", serde_json::to_string_pretty(&latest)?);

    // 3. The universal MCP bridge: every other verb is one call away.
    let listed: serde_json::Value = call_mcp(&client, "series.list", &json!({})).await?;
    println!("series in workspace: {listed}");

    Ok(())
}
