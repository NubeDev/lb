//! `case.assignees` — the assign picker's roster, and the disclosure wall that makes it safe.
//!
//! Part of the `plane` suite (see `plane_support.rs` for the fixtures and the full preamble). One
//! binary: `case_suite.rs`.
//!
//! The verb exists because the queue is a VIEWER surface and every verb that enumerates people is
//! admin-gated, so the one screen that needs a roster could not read one. It closes that by
//! answering the narrow question — *who do I share a team with?* — rather than handing a member the
//! workspace roster. These tests hold that line from both sides: what a caller on no team may learn
//! (nothing), and what a caller on a team may (that team, and its members).
//!
//! The POSITIVE gate test for the `case.list` alias lives in `plane_walls.rs` with its siblings, so
//! whoever adds the next aliased verb reads all of them at once.

use super::plane_support::*;

/// The privacy line. `seed_roster` puts priya on `team:mechanical` and deliberately leaves `test`
/// off it, so a caller who shares no team learns NOTHING: not that the team exists (that is admin
/// `teams.list`'s disclosure), not that priya is a member (that is `membership.list`'s).
///
/// This is the test that fails the day somebody "simplifies" the walk into `membership_list` +
/// `team_list` — both of which would answer, and both of which are admin-gated for exactly this
/// reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_learns_nothing_about_teams_they_are_not_on() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;

    let lister = principal("user:test", "nube", &[LIST]);
    let out = call(&node, &lister, "nube", "case.assignees", json!({}))
        .await
        .expect("the roster read is viewer-grade");

    assert_eq!(out["me"], json!("user:test"), "the caller is always named");
    assert_eq!(
        out["teams"],
        json!([]),
        "a team the caller is NOT on must not be disclosed: {out}"
    );
    assert_eq!(
        out["users"],
        json!([]),
        "a member sharing no team must not be disclosed: {out}"
    );
    // Said again over the whole payload, because the point is that the STRING never appears —
    // a future field carrying it would slip past the two assertions above.
    let blob = out.to_string();
    assert!(
        !blob.contains("mechanical") && !blob.contains("priya"),
        "case.assignees leaked a team or member the caller cannot see: {blob}"
    );
}

/// The positive roster: once the caller shares the team, they see it AND its members. Mirrors
/// `the_mine_lane_includes_the_teams_the_caller_is_on` — the same team graph, read for a different
/// question.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_member_sees_their_own_teams_and_the_people_on_them() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    lb_assets::relate(&node.store, "nube", MEMBER, "team:mechanical", "user:test")
        .await
        .expect("test joins the crew");

    let lister = principal("user:test", "nube", &[LIST]);
    let out = call(&node, &lister, "nube", "case.assignees", json!({}))
        .await
        .expect("the roster read is viewer-grade");

    assert_eq!(
        out["teams"],
        json!([{ "team": "team:mechanical", "name": "Mechanical crew" }]),
        "the caller's own team, with the display name a picker can render: {out}"
    );
    assert_eq!(
        out["users"],
        json!(["user:priya"]),
        "the teammate is offered, and the caller is NOT duplicated into the list: {out}"
    );
}

/// MANDATORY: workspace isolation. The team graph is ws-namespaced, so a ws-B caller on a
/// same-named team sees ws-B's members and nothing of ws-A's.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_roster_stops_at_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    lb_assets::relate(&node.store, "nube", MEMBER, "team:mechanical", "user:test")
        .await
        .expect("test joins the nube crew");

    // ws-B: the same team id, a different roster, and the caller is on it.
    membership_add_raw(&node.store, "acme", "user:test", 1)
        .await
        .expect("test joins acme");
    membership_add_raw(&node.store, "acme", "user:dana", 1)
        .await
        .expect("dana joins acme");
    team_create(&node.store, "acme", "team:mechanical", "Acme crew")
        .await
        .expect("acme team created");
    for who in ["user:test", "user:dana"] {
        lb_assets::relate(&node.store, "acme", MEMBER, "team:mechanical", who)
            .await
            .expect("acme crew");
    }

    let in_b = principal("user:test", "acme", &[LIST]);
    let out = call(&node, &in_b, "acme", "case.assignees", json!({}))
        .await
        .expect("the roster read is viewer-grade");

    assert_eq!(
        out["users"],
        json!(["user:dana"]),
        "ws-B sees only ws-B's roster — priya is a ws-A member: {out}"
    );
    assert_eq!(
        out["teams"][0]["name"],
        json!("Acme crew"),
        "the team record is the workspace's own, not ws-A's same-named one: {out}"
    );
}

/// The invariant that actually matters for the UI: every subject the picker offers is one the WRITE
/// accepts. A prefix mismatch between the two halves (`team:mechanical` vs a bare `mechanical`)
/// would ship a picker whose every team row fails on click, and no other test here would see it.
///
/// Note this is one-directional on purpose — `validate_assignee` accepts a strictly WIDER set than
/// `case.assignees` returns (any live member, any existing team), because assigning across teams is
/// a normal act. `case.assignees` is a suggestion list, not an allow-list; do not "tighten" the
/// write to it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_subject_the_picker_offers_is_one_the_write_accepts() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed_roster(&node, "nube").await;
    lb_assets::relate(&node.store, "nube", MEMBER, "team:mechanical", "user:test")
        .await
        .expect("test joins the crew");

    let full = principal("user:test", "nube", ALL);
    let insight = seed_insight(&node, &full, "nube", "picker-roundtrip", 1).await;
    let case_id = case_of(&node, &full, "nube", &insight).await;

    let out = call(&node, &full, "nube", "case.assignees", json!({}))
        .await
        .expect("the roster read");

    let mut offered: Vec<String> = vec![out["me"].as_str().expect("me").to_string()];
    for t in out["teams"].as_array().expect("teams") {
        offered.push(t["team"].as_str().expect("team subject").to_string());
    }
    for u in out["users"].as_array().expect("users") {
        offered.push(u.as_str().expect("user subject").to_string());
    }
    assert!(
        offered.len() >= 3,
        "fixture should offer me + a team + a teammate: {offered:?}"
    );

    for subject in offered {
        call(
            &node,
            &full,
            "nube",
            "case.assign",
            json!({ "id": case_id, "assignee": subject }),
        )
        .await
        .unwrap_or_else(|e| panic!("case.assign refused an offered subject {subject}: {e:?}"));
    }
}
