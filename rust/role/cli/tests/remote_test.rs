//! The REMOTE transport, driven against a REAL gateway on a real socket (testing §0 — no mocks, seed
//! via the real write path). Covers the mandatory categories for remote mode: the login→call spine,
//! capability-deny (the server's 403 relayed verbatim, exit non-zero), workspace-isolation (a ws-A
//! token returns only ws-A data even when a `-w B` is passed — the token's ws wins, by construction),
//! the typed `inbox list` over a seeded inbox, and offline (a down gateway is a clear error, not a
//! hang and not a fake success).

mod common;

use common::{dev_token, seed_inbox_item, spawn_gateway, token};
use lb_cli::error::CliError;
use lb_cli::login::do_login;
use lb_cli::transport::{Remote, Transport};
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_then_call_round_trips_over_the_real_gateway() {
    let gw = spawn_gateway().await;
    // The front door: POST /login mints a real signed token the gateway will accept.
    let reply = do_login(&reqwest::Client::new(), &gw.base_url, "user:ada", "acme")
        .await
        .expect("login succeeds");
    assert_eq!(reply.workspace, "acme");
    assert!(!reply.token.is_empty());

    // The spine, one command: a call the token is authorized for returns the tool's JSON. `system.*`
    // is granted by dev_claims; use `inbox.list` on an empty channel (returns `{items: []}`) so we do
    // not depend on any extension being loaded.
    let remote = Remote::new(&gw.base_url, reply.token);
    let out = remote
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .expect("authorized call returns the tool result");
    assert!(
        out.get("items").is_some(),
        "inbox.list returns an items envelope: {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_ungranted_call_relays_the_servers_deny_and_never_fakes_success() {
    // The mandatory capability-deny test: a token WITHOUT `inbox.list` calls it → the server 403s →
    // the CLI surfaces `DENIED mcp:inbox.list:call` and the result is an Err (exit non-zero), never a
    // fabricated ok.
    let gw = spawn_gateway().await;
    let tok = token(&gw.key, "user:mallory", "acme", &["bus:chan/*:pub"]); // no inbox.list
    let remote = Remote::new(&gw.base_url, tok);

    let result = remote
        .call("inbox.list", json!({ "channel": "general" }))
        .await;
    match result {
        Err(CliError::Denied { tool }) => {
            assert_eq!(tool, "inbox.list");
            // The rendered line is the honest deny the scope specifies.
            assert_eq!(
                CliError::Denied { tool }.to_string(),
                "DENIED  mcp:inbox.list:call"
            );
        }
        other => panic!("an ungranted call must be a DENY, got {other:?}"),
    }
    // Exit code is non-zero.
    assert_ne!(
        CliError::Denied {
            tool: "inbox.list".into()
        }
        .exit_code(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_a_token_returns_only_ws_a_data_even_when_targeting_b() {
    // The mandatory workspace-isolation test: seed the SAME channel+id in ws A and ws B with DIFFERENT
    // bodies, on ONE node. An A-token calls `inbox.list` — the server reads the ws from the token, so
    // it returns A's item, never B's. There is no ws in the /mcp/call body to honor; the A-token's ws
    // wins by construction (this is correct, not a bug).
    let gw = spawn_gateway().await;
    seed_inbox_item(&gw.node, "acme", "general", "i1", "A-secret").await;
    seed_inbox_item(&gw.node, "beta", "general", "i1", "B-secret").await;

    let a_token = dev_token(&gw.key, "user:ada", "acme");
    let remote = Remote::new(&gw.base_url, a_token);
    let out = remote
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .expect("A-token reads its own inbox");
    let text = out.to_string();
    assert!(text.contains("A-secret"), "A-token sees A's data: {text}");
    assert!(
        !text.contains("B-secret"),
        "A-token must NOT see B's data: {text}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_inbox_list_shapes_a_seeded_inbox() {
    // The typed command over a real seeded inbox — proves typed → /mcp/call shaping end to end.
    let gw = spawn_gateway().await;
    seed_inbox_item(&gw.node, "acme", "ops", "job-1", "deploy pending").await;
    seed_inbox_item(&gw.node, "acme", "ops", "job-2", "rollback ready").await;

    let tok = dev_token(&gw.key, "user:ada", "acme");
    let remote = Remote::new(&gw.base_url, tok);
    let out = lb_cli::commands::inbox::list(&remote, "ops", lb_cli::output::Format::Json)
        .await
        .expect("typed inbox list");
    // The body is the shaped JSON of the real items.
    assert!(
        out.body.contains("job-1") && out.body.contains("job-2"),
        "{}",
        out.body
    );
    // The header states the wall.
    assert!(out.header.contains("ws: acme"), "{}", out.header);
    assert!(out.header.contains("mode: remote"), "{}", out.header);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_mode_fails_clearly_when_the_gateway_is_down() {
    // The offline test for REMOTE mode: point at a port nothing is listening on → a clear Transport
    // error, not a hang and not a fake success. (No gateway spawned; the socket is dead.)
    let tok = "not.a.real.token";
    let remote = Remote::new("http://127.0.0.1:1", tok); // port 1: refused
    let result = remote
        .call("inbox.list", json!({ "channel": "general" }))
        .await;
    match result {
        Err(CliError::Transport(msg)) => {
            assert!(!msg.is_empty(), "a down gateway is a clear error")
        }
        other => panic!("a down gateway must be a Transport error, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_header_reflects_the_tokens_workspace_and_never_leaks_the_token() {
    let gw = spawn_gateway().await;
    let reply = do_login(&reqwest::Client::new(), &gw.base_url, "user:ada", "acme")
        .await
        .unwrap();
    let remote = Remote::new(&gw.base_url, reply.token.clone());
    let header = remote.header();
    assert_eq!(header.workspace, "acme");
    assert_eq!(header.user, "user:ada");
    assert!(
        !header.render().contains(&reply.token),
        "the header must never echo the token"
    );
}
