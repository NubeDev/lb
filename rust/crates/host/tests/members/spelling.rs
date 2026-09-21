//! Both spellings of a team id, across the membership verbs.
//!
//! Part of the `members` suite (see `support.rs` for the fixtures and the full preamble). One
//! binary: `members_suite.rs`.
//!
//! **Why this file exists.** `team_create` stores the id it is handed, VERBATIM — it normalises
//! nothing — so `mechanical` and `team:mechanical` are both real ids, and the product writes both:
//! the admin console posts a bare id while the pickers and `case.assign` speak the prefixed form.
//! Every reader of the `member` edge therefore has to accept both, or it silently misses the edges
//! written under the other one.
//!
//! `case.assignees` learned that (`insight::team_member_edges`); `members.list` and
//! `members.remove` did not, and the split showed up as two verbs disagreeing about the same team.
//! Neither failure is loud — `list` answers `{"members":[]}` and `remove` answers `ok:true` — which
//! is precisely why they need a test rather than a bug report.

use super::support::*;

/// `members.list` must find the edge whichever spelling it is asked for.
///
/// The asymmetry is the bug: `members.add` writes under the literal id it is given, so an edge
/// written bare and read prefixed (or the reverse) went missing with no error to explain it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_resolves_a_team_under_either_spelling() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;
    let admin = principal("user:test", "nube", &[ADD, M_LIST]);

    // Written under the BARE id — what the admin console posts.
    call(
        &node,
        &admin,
        "nube",
        "members.add",
        json!({ "team": "mechanical", "user": "user:priya" }),
    )
    .await
    .expect("add ok");

    for asked in ["mechanical", "team:mechanical"] {
        let out = call(
            &node,
            &admin,
            "nube",
            "members.list",
            json!({ "team": asked }),
        )
        .await
        .expect("list ok");
        assert_eq!(
            out["members"],
            json!(["user:priya"]),
            "asked for {asked}: a spelling must not hide a real membership: {out}"
        );
    }
}

/// …and the same the other way round, so the fix is not just "prefix wins".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_resolves_a_prefixed_write_asked_bare() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    seed(&node, "nube").await;
    let admin = principal("user:test", "nube", &[ADD, M_LIST]);

    call(
        &node,
        &admin,
        "nube",
        "members.add",
        json!({ "team": "team:mechanical", "user": "user:priya" }),
    )
    .await
    .expect("add ok");

    let out = call(
        &node,
        &admin,
        "nube",
        "members.list",
        json!({ "team": "mechanical" }),
    )
    .await
    .expect("list ok");
    assert_eq!(
        out["members"],
        json!(["user:priya"]),
        "a prefixed write must be readable bare: {out}"
    );
}

/// `members.remove` must actually drop the edge when the caller names the other spelling.
///
/// This is the one with teeth. Remove is idempotent, so a remove that matched nothing returns
/// `ok:true` exactly like a real one — the UI reports the member gone while the edge survives and
/// keeps handing them team-granted reads. The assertion is the LIST afterwards, not the ack.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn remove_drops_the_edge_under_either_spelling() {
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

    // Removed under the OTHER spelling than it was written with.
    call(
        &node,
        &admin,
        "nube",
        "members.remove",
        json!({ "team": "mechanical", "user": "user:priya" }),
    )
    .await
    .expect("remove ok");

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
        "the ack said ok; the edge must actually be gone: {out}"
    );
}
