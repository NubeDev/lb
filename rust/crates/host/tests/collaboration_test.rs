//! Host-layer tests for the collaboration services (collaboration scope): the channel registry,
//! members, inbox, outbox-status, and workspace directory verbs. Mandatory categories at the host
//! surface: **capability-deny** (each verb refused without its grant, before any store access) and
//! **workspace-isolation** (a ws-B principal can never see ws-A's records).
//!
//! These verbs back the gateway routes one-to-one; the gateway tests prove the same over HTTP. Here
//! the focus is the host capability chokepoint + the store wall directly.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    add_team_member, channel_create, channel_list, list_inbox, list_members, outbox_status,
    resolve_inbox, workspace_create, workspace_list, Node,
};
use lb_inbox::{Decision, Item};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const ALL: &[&str] = &[
    "bus:chan/*:pub",
    "bus:chan/*:sub",
    "mcp:members.list:call",
    "mcp:members.add:call",
    "mcp:inbox.list:call",
    "mcp:inbox.resolve:call",
    "mcp:outbox.status:call",
    "mcp:workspace.list:call",
    "mcp:workspace.create:call",
];

// ----- capability deny --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_verb_is_refused_without_its_grant() {
    let ws = "collab-deny";
    let node = Node::boot().await.expect("node boots");
    let p = principal(ws, &[]); // no caps at all

    assert!(channel_create(&node.store, &p, ws, "general", 1)
        .await
        .is_err());
    assert!(channel_list(&node.store, &p, ws).await.is_err());
    assert!(list_members(&node.store, &p, ws, "eng").await.is_err());
    assert!(add_team_member(&node.store, &p, ws, "eng", "user:x")
        .await
        .is_err());
    assert!(list_inbox(&node.store, &p, ws, "triage").await.is_err());
    assert!(
        resolve_inbox(&node.store, &p, ws, "i1", Decision::Approved, 1)
            .await
            .is_err()
    );
    assert!(outbox_status(&node.store, &p, ws).await.is_err());
    assert!(workspace_list(&node.store, &p).await.is_err());
    assert!(workspace_create(&node.store, &p, "new-ws", "New", 1)
        .await
        .is_err());
}

// ----- workspace isolation ----------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn channel_registry_is_workspace_isolated() {
    let node = Node::boot().await.expect("node boots");
    let a = principal("ws-a", ALL);
    let b = principal("ws-b", ALL);

    channel_create(&node.store, &a, "ws-a", "secret-room", 1)
        .await
        .expect("a creates a channel");

    let b_list = channel_list(&node.store, &b, "ws-b")
        .await
        .expect("b lists");
    assert!(b_list.is_empty(), "ISO LEAK: ws-b saw ws-a's channel");

    let a_list = channel_list(&node.store, &a, "ws-a")
        .await
        .expect("a lists");
    assert_eq!(a_list.len(), 1, "ws-a sees its own channel");
    assert_eq!(a_list[0].id, "secret-room");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_are_workspace_isolated() {
    let node = Node::boot().await.expect("node boots");
    let a = principal("ws-a", ALL);
    let b = principal("ws-b", ALL);

    add_team_member(&node.store, &a, "ws-a", "eng", "user:ada")
        .await
        .expect("a adds a member");

    let b_members = list_members(&node.store, &b, "ws-b", "eng")
        .await
        .expect("b lists");
    assert!(b_members.is_empty(), "ISO LEAK: ws-b saw ws-a's member");

    let a_members = list_members(&node.store, &a, "ws-a", "eng")
        .await
        .expect("a lists");
    assert_eq!(a_members, vec!["user:ada".to_string()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_is_workspace_isolated_and_resolve_records_the_actor() {
    let node = Node::boot().await.expect("node boots");
    let a = principal("ws-a", ALL);
    let b = principal("ws-b", ALL);

    lb_inbox::record(
        &node.store,
        "ws-a",
        &Item::new("appr-1", "approvals", "ext:gh", "needs:approval", 1),
    )
    .await
    .expect("seed ws-a item");

    let b_items = list_inbox(&node.store, &b, "ws-b", "approvals")
        .await
        .expect("b lists");
    assert!(b_items.is_empty(), "ISO LEAK: ws-b read ws-a's inbox");

    let a_items = list_inbox(&node.store, &a, "ws-a", "approvals")
        .await
        .expect("a lists");
    assert_eq!(a_items.len(), 1);

    // Resolve forces the actor to the session principal (`user:test`), never caller-supplied.
    resolve_inbox(&node.store, &a, "ws-a", "appr-1", Decision::Approved, 2)
        .await
        .expect("a resolves");
    let res = lb_inbox::resolution(&node.store, "ws-a", "appr-1")
        .await
        .expect("read")
        .expect("exists");
    assert_eq!(res.decision, Decision::Approved);
    assert_eq!(res.actor, "user:test");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn outbox_status_is_workspace_isolated() {
    let node = Node::boot().await.expect("node boots");
    let a = principal("ws-a", ALL);
    let b = principal("ws-b", ALL);

    let effect = lb_outbox::Effect::new("e1", "github", "create_pr", "{}", "idem-1", 1);
    lb_outbox::enqueue(
        &node.store,
        "ws-a",
        "side",
        "x",
        &serde_json::json!({}),
        &effect,
    )
    .await
    .expect("enqueue in ws-a");

    let b_status = outbox_status(&node.store, &b, "ws-b")
        .await
        .expect("b reads");
    assert!(
        b_status.pending.is_empty(),
        "ISO LEAK: ws-b saw ws-a's effect"
    );

    let a_status = outbox_status(&node.store, &a, "ws-a")
        .await
        .expect("a reads");
    assert_eq!(a_status.pending.len(), 1, "ws-a sees its own effect");
}
