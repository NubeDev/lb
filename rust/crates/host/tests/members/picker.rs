//! The assign picker's roster must agree with the assign it feeds (`case.assignees`).
//!
//! Part of the `members` suite (see `support.rs` for the fixtures and the full preamble). One
//! binary: `members_suite.rs`.
//!
//! Two ways the two sides disagreed, each returning a roster the operator could not act on: a team
//! id spelled one way and its `member` edges the other, and a roster offering subjects
//! `validate_assignee` refuses.

use super::support::*;

/// The bare-vs-prefixed team id, which is the SECOND half of the empty-picker bug and the half that
/// survives a working `members.add`.
///
/// A team record keeps whatever id it was created with, verbatim — and the product creates both
/// spellings: the admin console posts a BARE id (`mechanical`), while the pickers and `case.assign`
/// speak the prefixed one (`team:mechanical`). The `member` edge is keyed by that same raw string,
/// so a walk that looked up only one spelling found no members, concluded the caller was on no team,
/// and returned an empty roster — with no error anywhere.
///
/// This is the shape of a REAL workspace: a bare team record, a prefixed member edge. It failed
/// before `team_member_edges` accepted both.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_bare_team_id_still_finds_its_prefixed_member_edges() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    membership_add_raw(&node.store, "nube", "user:test", 1)
        .await
        .expect("test joins");
    // Created BARE, exactly as the admin console's `POST /admin/teams` does.
    team_create(&node.store, "nube", "mechanical", "Mechanical crew")
        .await
        .expect("team created");

    let admin = principal("user:test", "nube", &[ADD, C_LIST]);
    // Joined PREFIXED, exactly as a picker-shaped client does.
    call(
        &node,
        &admin,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:test" }),
    )
    .await
    .expect("add ok");

    let out = call(&node, &admin, "nube", "case.assignees", json!({}))
        .await
        .expect("assignees ok");
    assert_eq!(
        out["teams"],
        json!([{ "team": "team:mechanical", "name": "Mechanical crew" }]),
        "a bare team record must still resolve its prefixed member edges: {out}"
    );
}

/// The picker must not offer a subject that `case.assign` then REFUSES.
///
/// A `member` edge is an unvalidated write — it can name anyone, including a subject who never
/// joined the workspace — while `validate_assignee` requires a live workspace membership. So the
/// two sides disagreed: the roster offered the row, clicking it answered "assignee is not a member
/// of this workspace", and the control contradicted itself in front of the operator.
///
/// The assertion is deliberately the ROUND TRIP rather than the filter: what matters is not that
/// the row is absent, it is that every row the picker offers can actually be assigned.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_picker_never_offers_a_subject_assign_would_refuse() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let admin = principal(
        "user:test",
        "nube",
        &[ADD, C_LIST, OPEN, WORKFLOW, RAISE, I_GET],
    );
    // `user:ghost` is on the team but never joined the workspace — the edge does not check.
    for user in ["user:test", "user:priya", "user:ghost"] {
        call(
            &node,
            &admin,
            "nube",
            "members.add",
            json!({ "team": "team:mechanical", "user": user }),
        )
        .await
        .expect("add ok");
    }

    let roster = call(&node, &admin, "nube", "case.assignees", json!({}))
        .await
        .expect("assignees ok");
    let offered: Vec<String> = roster["users"]
        .as_array()
        .expect("users is an array")
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();

    assert!(
        offered.contains(&"user:priya".to_string()),
        "a real workspace member must still be offered: {roster}"
    );
    assert!(
        !offered.contains(&"user:ghost".to_string()),
        "the picker offered a non-member `case.assign` would refuse: {roster}"
    );

    // The round trip: every offered subject actually assigns. This is the property; the filter is
    // only how it is met.
    let id = seed_case(&node, &admin, "nube").await;
    for subject in &offered {
        call(
            &node,
            &admin,
            "nube",
            "case.assign",
            json!({ "id": id, "assignee": subject, "ts": 10 }),
        )
        .await
        .unwrap_or_else(|e| panic!("the picker offered `{subject}` but assign refused it: {e:?}"));
    }
}

/// **The end-to-end point of the whole change.** The assign picker was the surface that showed the
/// gap, so this proves the repair at that altitude: with the edge written over MCP — the only way a
/// UI can write it — `case.assignees` stops returning an empty roster and names the team and the
/// teammate the picker draws.
///
/// Before the bridge existed there was no sequence of MCP calls that could reach this state.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_assign_picker_fills_once_a_team_can_be_joined_over_mcp() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;

    let admin = principal("user:test", "nube", &[ADD, C_LIST]);

    // The roster starts empty — the state every shipped node was frozen in.
    let before = call(&node, &admin, "nube", "case.assignees", json!({}))
        .await
        .expect("assignees is viewer-grade");
    assert_eq!(before["teams"], json!([]), "precondition: no team yet");

    for user in ["user:test", "user:priya"] {
        call(
            &node,
            &admin,
            "nube",
            "members.add",
            json!({ "team": "team:mechanical", "user": user }),
        )
        .await
        .expect("join over the bridge");
    }

    let after = call(&node, &admin, "nube", "case.assignees", json!({}))
        .await
        .expect("assignees ok");
    assert_eq!(
        after["teams"],
        json!([{ "team": "team:mechanical", "name": "Mechanical crew" }]),
        "the picker must offer the team the caller just joined: {after}"
    );
    assert_eq!(
        after["users"],
        json!(["user:priya"]),
        "the picker must offer the teammate, and NOT duplicate the caller: {after}"
    );
}
