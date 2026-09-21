//! `members.*` over the REAL MCP bridge — reachability, idempotency, and the two mandatory walls.
//!
//! Part of the `members` suite (see `support.rs` for the fixtures and the full preamble). One
//! binary: `members_suite.rs`.

use super::support::*;

/// The regression, stated at the altitude it broke: the round trip add → list, over the bridge.
///
/// `NotFound` is the specific failure this guards. It is NOT the same as `Denied`, and the
/// distinction is the whole point — a denial means the wall spoke, a `NotFound` means no wall was
/// ever consulted because nothing routed the name.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_add_and_list_are_reachable_over_mcp() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let admin = principal("user:test", "nube", &[ADD, M_LIST]);

    let added = call(
        &node,
        &admin,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:priya" }),
    )
    .await
    .expect("members.add must ROUTE — a `no such tool` here is the bug this test exists for");
    assert_eq!(added["ok"], json!(true));

    let out = call(
        &node,
        &admin,
        "nube",
        "members.list",
        json!({ "team": "team:mechanical" }),
    )
    .await
    .expect("members.list must route");
    assert_eq!(
        out["members"],
        json!(["user:priya"]),
        "the edge the bridge wrote must read back through the bridge: {out}"
    );
}

/// Idempotent, because a picker that adds somebody twice must not double the roster.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn adding_the_same_member_twice_is_one_edge() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;
    let admin = principal("user:test", "nube", &[ADD, M_LIST]);
    let args = json!({ "team": "team:mechanical", "user": "user:priya" });

    for _ in 0..2 {
        call(&node, &admin, "nube", "members.add", args.clone())
            .await
            .expect("add ok");
    }

    let out = call(
        &node,
        &admin,
        "nube",
        "members.list",
        json!({ "team": "team:mechanical" }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        out["members"],
        json!(["user:priya"]),
        "re-add duplicated the edge: {out}"
    );
}

/// Capability-deny (mandatory category). A caller without `members.add` is `Denied` — NOT
/// `NotFound`, which would mean the routing regressed rather than the wall holding.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_add_denies_without_the_cap() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let viewer = principal("user:test", "nube", &[M_LIST]);
    let err = call(
        &node,
        &viewer,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:priya" }),
    )
    .await
    .expect_err("a caller without members.add must not write the edge");
    assert!(
        matches!(err, ToolError::Denied),
        "expected Denied (the wall), got {err:?} — NotFound here means the dispatch arm is gone"
    );
}

/// The POSITIVE gate test for the `members.remove` → `teams.manage` alias, the category
/// `plane_walls.rs` exists for: a missing `tool_gate.rs` arm is `Denied`, not `NotFound`, so only a
/// test that calls it WITH the aliased cap and expects success can catch it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn members_remove_passes_the_gate_under_teams_manage() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let admin = principal("user:test", "nube", &[ADD, M_LIST, T_MANAGE]);
    call(
        &node,
        &admin,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:priya" }),
    )
    .await
    .expect("add ok");

    call(
        &node,
        &admin,
        "nube",
        "members.remove",
        json!({ "team": "team:mechanical", "user": "user:priya" }),
    )
    .await
    .expect("members.remove must pass BOTH gates under `teams.manage` alone");

    let out = call(
        &node,
        &admin,
        "nube",
        "members.list",
        json!({ "team": "team:mechanical" }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        out["members"],
        json!([]),
        "the edge survived removal: {out}"
    );
}

/// Workspace isolation (mandatory category): a ws-B admin cannot put anyone on a ws-A team, and the
/// ws-A list never shows their write.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_foreign_workspace_cannot_write_the_edge() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let outsider = principal("user:eve", "other", &[ADD, M_LIST]);
    let err = call(
        &node,
        &outsider,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:eve" }),
    )
    .await
    .expect_err("workspace-first must reject a cross-workspace write");
    assert!(
        matches!(err, ToolError::Denied),
        "expected Denied, got {err:?}"
    );

    let admin = principal("user:test", "nube", &[M_LIST]);
    let out = call(
        &node,
        &admin,
        "nube",
        "members.list",
        json!({ "team": "team:mechanical" }),
    )
    .await
    .expect("list ok");
    assert_eq!(out["members"], json!([]), "a foreign write landed: {out}");
}
