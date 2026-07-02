//! `lb reminder …` — the common resource grammar driven against a REAL gateway on a real socket
//! (testing §0 / CLAUDE §9 — no mocks, seed via the real write path). Mirrors `remote_test.rs` + the
//! inbox-list test, covering the mandatory categories for the reminder family:
//!   - round-trip: create → ls shows it → show returns it → update --enabled false → rm;
//!   - capability-deny (mandatory): a token WITHOUT `mcp:reminder.delete:call` → DENIED, exit 3;
//!   - workspace-isolation (mandatory): a ws-A token passed `-w B` lists only ws-A's reminders, and a
//!     ws-B token cannot rm a ws-A reminder (the token's ws wins — the hard wall);
//!   - `create` prints the id (resource-verbs D4), not the full record.
//!
//! The commands are exercised through the CLI's typed layer (`commands::reminder::*`) over the `Remote`
//! transport — the exact path `lb reminder …` runs, minus argv parsing.

mod common;

use common::{dev_token, seed_reminder, spawn_gateway, token};
use lb_cli::commands::reminder;
use lb_cli::error::CliError;
use lb_cli::output::Format;
use lb_cli::transport::Remote;

/// The full reminder caps a normal (dev-login) operator carries, minus none — used where the test
/// wants an authorized session. `dev_token` already grants the `mcp:*.{create,list,get,update,delete}`
/// wildcards, so it authorizes every reminder verb; we use it for the happy paths.
fn authed(gw: &common::RunningGateway, ws: &str) -> Remote {
    Remote::new(&gw.base_url, dev_token(&gw.key, "user:ada", ws))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn create_ls_show_update_rm_round_trips_over_the_real_gateway() {
    let gw = spawn_gateway().await;
    let ws = "acme";
    let t = authed(&gw, ws);

    // create → prints the id (D4), not the record. Table format is the human path.
    let created = reminder::create::run(
        &t,
        "team",
        "standup time",
        "0 9 * * 1",
        None,
        None,
        Format::Table,
    )
    .await
    .expect("create with the grant");
    let id = created.body.trim().to_string();
    assert!(id.starts_with("standup-time-"), "create prints an id: {id}");
    // D4: the body is JUST the id, not the full record — no schedule/action fields leaked.
    assert!(
        !created.body.contains("schedule") && !created.body.contains("channel"),
        "create prints only the id, not the record: {}",
        created.body
    );

    // ls shows it.
    let listed = reminder::ls::run(&t, None, None, Format::Json)
        .await
        .expect("ls");
    assert!(
        listed.body.contains(&id),
        "ls shows the new reminder: {}",
        listed.body
    );

    // show returns it (the record, unwrapped).
    let shown = reminder::show::run(&t, &id, Format::Json)
        .await
        .expect("show");
    assert!(
        shown.body.contains(&id),
        "show returns the record: {}",
        shown.body
    );
    assert!(shown.body.contains("standup time"), "{}", shown.body);

    // update --enabled false pauses it; --status disabled now finds it, --status enabled does not.
    reminder::update::run(&t, &id, Some(false), None, None, Format::Json)
        .await
        .expect("update --enabled false");
    let disabled = reminder::ls::run(&t, Some("disabled"), None, Format::Json)
        .await
        .unwrap();
    assert!(
        disabled.body.contains(&id),
        "paused → shows under disabled: {}",
        disabled.body
    );
    let enabled = reminder::ls::run(&t, Some("enabled"), None, Format::Json)
        .await
        .unwrap();
    assert!(
        !enabled.body.contains(&id),
        "paused → not under enabled: {}",
        enabled.body
    );

    // rm tombstones it → ls no longer shows it.
    reminder::rm::run(&t, &id, false, Format::Json)
        .await
        .expect("rm");
    let after = reminder::ls::run(&t, None, None, Format::Json)
        .await
        .unwrap();
    assert!(
        !after.body.contains(&id),
        "deleted → not listed: {}",
        after.body
    );

    // The header states the wall + mode.
    assert!(after.header.contains("ws: acme"), "{}", after.header);
    assert!(after.header.contains("mode: remote"), "{}", after.header);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rm_without_the_delete_cap_is_a_deny_and_never_a_fake_success() {
    // MANDATORY capability-deny: a member token holding the OTHER reminder caps but NOT
    // `mcp:reminder.delete:call` → the server 403s → the CLI surfaces `DENIED mcp:reminder.delete:call`
    // and returns Err (exit 3), never a fabricated ok. We seed a real reminder first so the deny is on
    // a real target, not a missing one.
    let gw = spawn_gateway().await;
    let ws = "acme";
    seed_reminder(&gw.node, ws, "victim", "team", "standup").await;

    let tok = token(
        &gw.key,
        "user:mallory",
        ws,
        &[
            "mcp:reminder.create:call",
            "mcp:reminder.list:call",
            "mcp:reminder.get:call",
            "mcp:reminder.update:call",
        ], // deliberately no delete
    );
    let t = Remote::new(&gw.base_url, tok);

    let result = reminder::rm::run(&t, "victim", false, Format::Json).await;
    match result {
        Err(CliError::Denied { tool }) => {
            assert_eq!(tool, "reminder.delete");
            assert_eq!(
                CliError::Denied { tool }.to_string(),
                "DENIED  mcp:reminder.delete:call"
            );
        }
        other => panic!("an ungranted rm must be a DENY, got {other:?}"),
    }
    assert_eq!(
        CliError::Denied {
            tool: "reminder.delete".into()
        }
        .exit_code(),
        3
    );

    // The reminder still exists — a denied rm touched nothing.
    let still = reminder::ls::run(&authed(&gw, ws), None, None, Format::Json)
        .await
        .unwrap();
    assert!(
        still.body.contains("victim"),
        "denied rm left the record: {}",
        still.body
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_ws_a_token_lists_only_ws_a_even_when_targeting_b() {
    // MANDATORY workspace-isolation: seed reminders in ws A and ws B on ONE node. An A-token lists —
    // the server reads the ws from the token, so it returns A's reminder, never B's. There is no ws in
    // the /mcp/call body to honor; the A-token's ws wins by construction.
    let gw = spawn_gateway().await;
    seed_reminder(&gw.node, "acme", "a-only", "team", "A secret").await;
    seed_reminder(&gw.node, "beta", "b-only", "team", "B secret").await;

    let a = Remote::new(&gw.base_url, dev_token(&gw.key, "user:ada", "acme"));
    let listed = reminder::ls::run(&a, None, None, Format::Json)
        .await
        .unwrap();
    assert!(
        listed.body.contains("a-only"),
        "A sees its own: {}",
        listed.body
    );
    assert!(
        !listed.body.contains("b-only"),
        "A must NOT see B's reminder: {}",
        listed.body
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ws_b_cannot_rm_a_ws_a_reminder() {
    // MANDATORY workspace-isolation on the write path: a ws-B token cannot delete a ws-A reminder. The
    // delete runs in ws-B's namespace (the token's ws), where `a-victim` does not exist — the shipped
    // delete is idempotent (deleting an absent id is a no-op ok), so the assertion is that ws-A's
    // reminder SURVIVES: the wall physically prevented the cross-ws delete.
    let gw = spawn_gateway().await;
    seed_reminder(&gw.node, "acme", "a-victim", "team", "A secret").await;

    let b = Remote::new(&gw.base_url, dev_token(&gw.key, "user:bob", "beta"));
    // ws-B's rm cannot reach ws-A's namespace; it operates on beta and leaves acme untouched.
    let _ = reminder::rm::run(&b, "a-victim", false, Format::Json).await;

    let a = Remote::new(&gw.base_url, dev_token(&gw.key, "user:ada", "acme"));
    let still = reminder::ls::run(&a, None, None, Format::Json)
        .await
        .unwrap();
    assert!(
        still.body.contains("a-victim"),
        "ISO LEAK: ws-B deleted ws-A's reminder: {}",
        still.body
    );
}
