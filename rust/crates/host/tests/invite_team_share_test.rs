//! A person who joins a team THROUGH AN INVITE reaches what is shared with that team.
//!
//! Regression: `invite_accept` wrote the team `member` edge with the BARE email (`sam@x.com`) while
//! every share check compared edges to the full sub (`user:sam@x.com`). The invitee was in the team
//! roster yet was handed none of its menus, boards or sites. Accept now writes the full sub, and the
//! share checks accept either spelling, so edges written before the fix keep working.
//!
//! Real embedded node, real invite accept, real nav/dashboard share — no mocks.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, invite_accept, invite_create, Node};
use lb_mcp::ToolError;
use serde_json::{json, Value};

const WS: &str = "estate";

fn principal(sub: &str, caps: &[String]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: WS.into(),
        role: Role::Member,
        caps: caps.to_vec(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

fn admin() -> Principal {
    let caps: Vec<String> = [
        "mcp:authz.entity_scope:call",
        "mcp:invite.create:call",
        "mcp:teams.manage:call",
        "mcp:teams.create:call",
        "mcp:members.add:call",
        "mcp:members.list:call",
        "mcp:nav.save:call",
        "mcp:nav.share:call",
        "mcp:dashboard.save:call",
        "mcp:dashboard.share:call",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();
    principal("user:admin", &caps)
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, WS, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// Team g1, with a menu that marks "Site A" and a board, both shared with g1.
async fn setup() -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin();
    for (tool, input) in [
        ("teams.create", json!({"team": "g1", "name": "Group 1"})),
        (
            "nav.save",
            json!({"id": "g1menu", "title": "Group 1", "now": 1, "items": [
                {"kind": "group", "label": "Site A", "entity": {"table": "site", "id": "Site A"}, "items": []}
            ]}),
        ),
        (
            "nav.share",
            json!({"id": "g1menu", "visibility": "team", "team": "g1", "now": 2}),
        ),
        (
            "dashboard.save",
            json!({"id": "g1board", "title": "Board", "now": 3, "cells": []}),
        ),
        (
            "dashboard.share",
            json!({"id": "g1board", "visibility": "team", "team": "g1", "now": 4}),
        ),
    ] {
        call(&node, &admin, tool, input).await.unwrap();
    }
    node
}

/// `p` is handed the team's menu, board and site.
async fn assert_reaches_the_team_share(node: &Arc<Node>, p: &Principal) {
    let scope = call(node, p, "authz.entity_scope", json!({"table": "site"}))
        .await
        .unwrap();
    assert_eq!(scope, json!({"filter": {"ids": ["Site A"]}}), "{}", p.sub());
    call(node, p, "nav.get", json!({"id": "g1menu"}))
        .await
        .unwrap_or_else(|e| panic!("{} cannot read the team menu: {e:?}", p.sub()));
    call(node, p, "dashboard.get", json!({"id": "g1board"}))
        .await
        .unwrap_or_else(|e| panic!("{} cannot read the team board: {e:?}", p.sub()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_invitee_joins_the_team_as_their_full_sub_and_reaches_its_shares() {
    let node = setup().await;
    let invite = invite_create(
        &node.store,
        &admin(),
        WS,
        "sam@example.com",
        "viewer",
        "g1",
        None,
        None,
        0,
        100,
    )
    .await
    .unwrap();
    let key = SigningKey::generate();
    let joined = invite_accept(&node.store, &key, WS, &invite, "password123", None, 200)
        .await
        .unwrap();

    let roster = call(&node, &admin(), "members.list", json!({"team": "g1"}))
        .await
        .unwrap();
    assert_eq!(roster, json!({"members": ["user:sam@example.com"]}));

    assert_reaches_the_team_share(&node, &principal(&joined.sub, &joined.caps)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_bare_member_edge_from_before_the_fix_still_reaches_the_shares() {
    let node = setup().await;
    // What an invite accepted before the fix left behind: the email without `user:`.
    call(
        &node,
        &admin(),
        "members.add",
        json!({"team": "g1", "user": "bob@example.com"}),
    )
    .await
    .unwrap();
    let caps: Vec<String> = [
        "mcp:authz.entity_scope:call",
        "mcp:nav.get:call",
        "mcp:dashboard.get:call",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();

    assert_reaches_the_team_share(&node, &principal("user:bob@example.com", &caps)).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn someone_outside_the_team_is_still_refused() {
    let node = setup().await;
    let caps: Vec<String> = [
        "mcp:authz.entity_scope:call",
        "mcp:nav.get:call",
        "mcp:dashboard.get:call",
    ]
    .iter()
    .map(|c| c.to_string())
    .collect();
    let outsider = principal("user:eve@example.com", &caps);

    let scope = call(
        &node,
        &outsider,
        "authz.entity_scope",
        json!({"table": "site"}),
    )
    .await
    .unwrap();
    assert_eq!(scope, json!({"filter": {"ids": []}}));
    assert!(call(&node, &outsider, "nav.get", json!({"id": "g1menu"}))
        .await
        .is_err());
    assert!(
        call(&node, &outsider, "dashboard.get", json!({"id": "g1board"}))
            .await
            .is_err()
    );
}
