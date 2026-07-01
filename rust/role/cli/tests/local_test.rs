//! The LOCAL transport, driven fully OFFLINE (no gateway anywhere) against an in-process node
//! (testing §0 — real store, real host, no mocks). Covers the mandatory categories for local mode:
//! offline operation with output identical to remote, capability-deny parity (local denies the same
//! verbs a member token would), and workspace-isolation (a local `-w acme` principal cannot reach ws
//! beta's data on the same store).

mod common;

use std::sync::Arc;

use lb_cli::error::CliError;
use lb_cli::output::Format;
use lb_cli::transport::{Local, Transport};
use serde_json::json;

/// Seed an inbox item directly on a node's store (the same real write path the gateway seed uses).
async fn seed(node: &lb_host::Node, ws: &str, channel: &str, id: &str, body: &str) {
    let principal =
        lb_auth::Principal::routed("user:seed", ws, vec!["mcp:inbox.record:call".to_string()]);
    lb_host::record_inbox(&node.store, &principal, ws, channel, id, body, 1000)
        .await
        .expect("seed via real write path");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_mode_runs_with_no_gateway_reachable() {
    // The offline posture: boot an in-process node, mint a dev principal for `acme`, call a verb — no
    // network at all. The result is the tool's real JSON.
    let local = Local::boot("user:ada", "acme")
        .await
        .expect("local node boots");
    let out = local
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .expect("offline call returns the tool result");
    assert!(
        out.get("items").is_some(),
        "inbox.list returns an items envelope offline: {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_output_matches_remote_shape() {
    // Local mints the SAME dev_claims a login issues, so the shaped output is identical to remote.
    let local = Local::boot("user:ada", "acme").await.unwrap();
    seed(local.node(), "acme", "ops", "job-1", "deploy pending").await;

    let out = lb_cli::commands::inbox::list(&local, "ops", Format::Json)
        .await
        .expect("typed inbox list offline");
    assert!(out.body.contains("job-1"), "{}", out.body);
    // The header marks it LOCAL so an operator is never confused about offline-ness.
    assert!(out.header.contains("mode: local"), "{}", out.header);
    assert!(out.header.contains("ws: acme"), "{}", out.header);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_denies_a_verb_outside_the_dev_claims_set() {
    // Parity deny test: local is NOT an admin backdoor — a verb NOT in dev_claims is denied exactly
    // like a member token would be. `prefs.set_default` is deliberately NOT in dev_claims (it writes
    // the workspace default — admin-only, ungranted to the dev member).
    let local = Local::boot("user:ada", "acme").await.unwrap();
    let result = local.call("prefs.set_default", json!({})).await;
    match result {
        Err(CliError::Denied { tool }) => assert_eq!(tool, "prefs.set_default"),
        other => panic!("local must deny an ungranted verb, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_dash_w_cannot_reach_outside_the_minted_principals_workspace() {
    // The isolation test for local: seed the SAME channel+id in acme and beta on ONE node, then mint
    // TWO local transports over that one node — one scoped to acme, one to beta. Each sees only its own
    // workspace's data. `-w` scoped the principal's ws (the wall); it cannot cross.
    let node = Arc::new(lb_host::Node::boot().await.expect("node boots"));
    seed(&node, "acme", "general", "i1", "A-secret").await;
    seed(&node, "beta", "general", "i1", "B-secret").await;

    let acme = Local::over(Arc::clone(&node), "user:ada", "acme");
    let out = acme
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .unwrap();
    let text = out.to_string();
    assert!(
        text.contains("A-secret") && !text.contains("B-secret"),
        "acme sees only A: {text}"
    );

    let beta = Local::over(Arc::clone(&node), "user:bo", "beta");
    let out = beta
        .call("inbox.list", json!({ "channel": "general" }))
        .await
        .unwrap();
    let text = out.to_string();
    assert!(
        text.contains("B-secret") && !text.contains("A-secret"),
        "beta sees only B: {text}"
    );
}
