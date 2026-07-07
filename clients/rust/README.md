# `lb-client` (Rust)

A thin external client for a Lazybones gateway node — **authenticate → connect →
round-trip a `Sample`**, plus the webhook third-party caller path and the
universal `POST /mcp/call` bridge. Deliberately small: the shape to extend, not
an SDK.

> Scope: [`docs/scope/clients/client-libraries-scope.md`](../../docs/scope/clients/client-libraries-scope.md).
> Wire reference: [`docs/skills/ingest-series/SKILL.md`](../../docs/skills/ingest-series/SKILL.md).
> This crate is **not** a member of the core `rust/Cargo.toml` workspace — it
> builds standalone.

## Install

The crate is unpublished (vendored in-repo). Pull it as a path/git dependency:

```toml
[dependencies]
lb-client = { path = "../clients/rust" }     # vendor
# lb-client = { git = "https://github.com/.../lb", subdirectory = "clients/rust" }
```

## Authenticate

The bearer is **either** an API key (`lbk_{ws}.{id}.{secret}`) **or** a JWT from
`/login`. The library doesn't care which — the gateway splits on the `lbk_`
prefix in one place. **Read the key from an env var; never hard-code it.**

```rust
use lb_client::Client;

// long-lived producer (recommended): an API key minted via the admin console
let client = Client::new("http://127.0.0.1:8080", std::env::var("LB_KEY")?);

// or dev/admin script: log in to get a 12h session token
let client = Client::new("http://127.0.0.1:8080", "placeholder");
let (client, reply) = client.login("ada", "acme").await?;
```

## The round-trip

```rust
use lb_client::{write_samples, latest_sample};
use serde_json::json;

let written = write_samples(&client, vec![json!({
    "series": "node.cpu_temp",
    "ts": 1719800000000_u64,
    "seq": 1,
    "payload": 61.4,
    "labels": { "host": "pi-7" },
    // producer is host-forced to the authenticated principal; omit it
})]).await?;
// accepted=1 committed=1   (the gateway drains staging on the same call)

let latest = latest_sample(&client, "node.cpu_temp").await?;
// latest.sample.payload == 61.4
```

## The universal MCP bridge

Every other platform verb — `series.list`, `series.find`, `inbox.read`,
`channel.post`, … — is one `call_mcp` away without a library update:

```rust
use lb_client::call_mcp;
use serde_json::json;

let series: serde_json::Value =
    call_mcp(&client, "series.list", &json!({"prefix": "node."})).await?;
```

## Webhook (the third-party caller path)

A service the admin has shared a webhook secret with signs the raw body and
POSTs to `/hooks/{ws}/{id}`. **Sign the exact bytes you POST.**

```rust
use lb_client::{sign_webhook, post_webhook};

let body = br#"{"event":"furnace-on"}"#.to_vec();
let sig  = sign_webhook(shared_secret.as_bytes(), &body); // sha256=<hex>
let accepted = post_webhook(
    &client, "acme", "wh_x",
    &[("X-Signature".into(), sig)],
    body,
).await?;
```

## Errors

`LbError::Api(ApiError { status, body })` carries the gateway's status + body
verbatim. `is_denied()` covers the opaque `401|403|404` statuses the gateway
returns for missing-cap / cross-workspace / unknown-record (the contract never
distinguishes them):

```rust
use lb_client::{ApiError, LbError};

match write_samples(&client, samples).await {
    Ok(r) => r,
    Err(LbError::Api(api)) if api.is_denied() => {
        // opaque deny — missing cap, cross-workspace, or unknown record
        return Err(api.into());
    }
    Err(e) => return Err(e.into()),
}
```

## Run the example

```bash
make cloud                              # terminal 1: boot 127.0.0.1:8080
cd clients/rust
LB_URL=http://127.0.0.1:8080 LB_USER=ada LB_WORKSPACE=acme cargo run --example roundtrip
# or with an API key:
LB_KEY=lbk_acme.k7f3a.ABCDEF23 cargo run --example roundtrip
```

## Lay of the land

One verb per file (≤150 lines), per the project's FILE-LAYOUT rule:

```
src/
  lib.rs        — barrel re-export
  client.rs     — Client + login() + the shared HTTP plumbing
  error.rs      — ApiError + LbError (deny is opaque)
  ingest.rs     — write_samples() + latest_sample()
  mcp.rs        — call_mcp() (universal bridge)
  webhook.rs    — sign_webhook() + post_webhook()
examples/
  roundtrip.rs  — login → write → read demo
```
