//! Config persistence ACROSS invocations, over a REAL gateway (testing §0). Proves the front-door
//! contract: `lb login` stores a token that a *later, separate* command loads and uses — the whole
//! point of the per-workspace credential slot. Also pins the `-w <unstored>` loud-error path and the
//! token-custody discipline (the stored config's token is never surfaced by a command).

mod common;

use common::spawn_gateway;
use lb_cli::config::{load_from, save_to, Config};
use lb_cli::context::RunContext;
use lb_cli::error::CliError;
use lb_cli::login::do_login;
use lb_cli::transport::Transport;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn login_persists_a_credential_a_later_command_loads_and_uses() {
    let gw = spawn_gateway().await;
    let dir = tempfile::tempdir().unwrap();
    let cfg_path = dir.path().join("config");

    // Invocation 1: login → store the token in the config (as `lb login` does).
    let reply = do_login(&reqwest::Client::new(), &gw.base_url, "user:ada", "acme")
        .await
        .unwrap();
    let mut cfg = Config::default();
    cfg.gateway_url = Some(gw.base_url.clone());
    cfg.set_token(&reply.workspace, reply.token.clone());
    save_to(&cfg, &cfg_path).unwrap();

    // Invocation 2 (separate): a fresh load of the config selects the stored credential and calls.
    let loaded = load_from(&cfg_path).unwrap();
    let ctx = RunContext {
        workspace: Some("acme".into()),
        gateway_url: gw.base_url.clone(),
        local: false,
        config: loaded,
    };
    let remote = ctx.remote().expect("stored credential selected");
    let out = remote
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .expect("the persisted token authenticates a later command");
    assert!(out.get("items").is_some(), "{out}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn selecting_an_unstored_workspace_errors_loudly() {
    // `-w gamma` with only an `acme` credential stored → a loud NoCredential error, never a silent
    // hop to another workspace (the load-bearing `-w` credential-selector rule).
    let mut cfg = Config::default();
    cfg.set_token("acme", "tok-acme");
    let ctx = RunContext {
        workspace: Some("gamma".into()),
        gateway_url: "http://127.0.0.1:1".into(),
        local: false,
        config: cfg,
    };
    match ctx.remote() {
        Err(CliError::NoCredential { workspace }) => {
            assert_eq!(workspace, "gamma");
            assert!(CliError::NoCredential { workspace }
                .to_string()
                .contains("run `lb login -w gamma`"));
        }
        other => panic!("expected a loud NoCredential, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_command_output_ever_emits_the_stored_token() {
    // The token-custody regression: after a real login, no command's rendered output (header + body)
    // contains the secret token. Runs `whoami` (which reads the token) and `inbox list` and asserts the
    // token appears in neither.
    let gw = spawn_gateway().await;
    let reply = do_login(&reqwest::Client::new(), &gw.base_url, "user:ada", "acme")
        .await
        .unwrap();
    let remote = lb_cli::transport::Remote::new(&gw.base_url, reply.token.clone());

    // whoami: header + caps body.
    let header = remote.header();
    let caps = remote.caps();
    let who =
        lb_cli::commands::whoami::render(&header, &caps, lb_cli::output::Format::Json).unwrap();
    assert!(
        !who.header.contains(&reply.token) && !who.body.contains(&reply.token),
        "whoami leaked the token"
    );

    // inbox list: header + shaped body.
    let listing = lb_cli::commands::inbox::list(&remote, "general", lb_cli::output::Format::Json)
        .await
        .unwrap();
    assert!(
        !listing.header.contains(&reply.token) && !listing.body.contains(&reply.token),
        "inbox list leaked the token"
    );
}
