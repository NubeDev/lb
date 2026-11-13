//! `case.escalate` — hand a case to a workspace member or team and mail them a link to it
//! (`docs/scope/insights/case-escalate-scope.md`).
//!
//! Part of the `request` suite (it shares `request_support.rs`'s fixtures: the real relay, the real
//! email target, the recording provider). One binary: `case_suite.rs`.
//!
//! What is held here: the assign AND the mail both happen, one mail per distinct address, the
//! history says so without an address in it, and every refusal — unknown subject, no address, no
//! grant, out-of-scope case — leaves the case exactly as it was.

use lb_authz::{identity_create, identity_create_with_email, team_create, MEMBER};

use super::request_support::*;

/// The caps the verb needs and nothing else: the ask grant (the alias) and the triage grant (the
/// assign half). Holding ONLY these is the positive proof the `tool_gate.rs` arm exists.
const ESCALATE: &[&str] = &[SEND, WORKFLOW];

/// `user:test` (no identity, no address), `user:priya` (an identity with a mixed-case login email),
/// and `user:sam@example.com` (an email-shaped sub with no identity row) — all workspace members.
async fn seed_people(node: &Arc<Node>, ws: &str) {
    seed_roster(node, ws).await;
    for who in ["user:priya", "user:sam@example.com", "user:quiet"] {
        membership_add_raw(&node.store, ws, who, 1)
            .await
            .expect("joins");
    }
    identity_create_with_email(
        &node.store,
        "user:priya",
        Some("Priya"),
        "Priya@Example.com",
        1,
    )
    .await
    .expect("priya has a login email");
    identity_create(&node.store, "user:quiet", Some("Quiet"), 1)
        .await
        .expect("quiet has no email");
}

async fn escalate(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    input: Value,
) -> Result<Value, ToolError> {
    call(node, p, ws, "case.escalate", input).await
}

async fn owner_of(node: &Arc<Node>, ws: &str, case_id: &str) -> Value {
    let full = principal("user:test", ws, ALL);
    let case = call(node, &full, ws, "case.get", json!({ "id": case_id }))
        .await
        .expect("get ok");
    case["assigned_to"].clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_user_escalation_assigns_mails_once_and_records_it_without_the_address() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_people(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "escalate-user").await;

    let caller = principal("user:test", ws, ESCALATE);
    let out = escalate(
        &node,
        &caller,
        ws,
        json!({
            "case_id": case_id,
            "to": "user:priya",
            "subject": "Over to you",
            "message": "Please check <gate 3>.",
            "ts": T0,
        }),
    )
    .await
    .expect("a caller holding only the ask + triage grants may escalate");

    assert_eq!(out["recipients"], json!(1), "{out}");
    assert_eq!(out["case"]["assigned_to"], json!("user:priya"), "{out}");
    assert!(
        !out.to_string().contains('@'),
        "no address in the reply: {out}"
    );
    assert_eq!(owner_of(&node, ws, &case_id).await, json!("user:priya"));

    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "escalated"), 1);
    let row = events.iter().find(|e| e["kind"] == "escalated").unwrap();
    assert_eq!(row["actor"], json!("user:test"));
    assert_eq!(row["data"]["to"], json!("user:priya"));
    assert_eq!(row["data"]["recipients"], json!(1));
    assert_eq!(row["data"]["subject"], json!("Over to you"));
    assert!(
        !row.to_string().contains('@'),
        "no address in history: {row}"
    );

    let sends = drain_recording(&node, ws).await.sends();
    assert_eq!(sends.len(), 1, "exactly one mail");
    assert_eq!(
        sends[0].to, "priya@example.com",
        "the identity's login email"
    );
    assert_eq!(sends[0].subject, "Over to you");
    assert!(
        sends[0].body.starts_with("Please check <gate 3>."),
        "{}",
        sends[0].body
    );
    assert!(
        sends[0]
            .body
            .contains(&format!("/#/t/nube/insights?tab=cases&case={case_id}")),
        "{}",
        sends[0].body
    );
    let html = sends[0].html.clone().unwrap_or_default();
    assert!(
        html.contains("&lt;gate 3&gt;") && !html.contains("<gate"),
        "{html}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_team_escalation_mails_each_live_member_once() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_people(&node, ws).await;
    team_create(&node.store, ws, "team:crew", "Crew")
        .await
        .expect("team");
    // priya under BOTH edge spellings (one address), sam by an email-shaped sub, quiet with no
    // address, and a ghost who is on the team but not in the workspace.
    identity_create_with_email(&node.store, "user:ghost", None, "ghost@example.com", 1)
        .await
        .expect("ghost identity");
    for who in [
        "user:priya",
        "priya",
        "user:sam@example.com",
        "user:quiet",
        "user:ghost",
    ] {
        lb_assets::relate(&node.store, ws, MEMBER, "team:crew", who)
            .await
            .expect("crew edge");
    }
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "escalate-team").await;

    let out = escalate(
        &node,
        &principal("user:test", ws, ESCALATE),
        ws,
        json!({ "case_id": case_id, "to": "team:crew", "ts": T0 }),
    )
    .await
    .expect("team escalation ok");
    assert_eq!(out["recipients"], json!(2), "{out}");
    assert_eq!(out["case"]["assigned_to"], json!("team:crew"));

    let sends = drain_recording(&node, ws).await.sends();
    let mut to: Vec<&str> = sends.iter().map(|s| s.to.as_str()).collect();
    to.sort();
    assert_eq!(to, ["priya@example.com", "sam@example.com"]);
    assert!(
        sends
            .iter()
            .all(|s| s.subject.starts_with("Escalated to you: ")),
        "the default subject when none was typed"
    );
}

/// The refusals change nothing: no owner, no mail, no history row.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_refused_escalation_changes_nothing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_people(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "escalate-refused").await;
    let before = owner_of(&node, ws, &case_id).await;
    let caller = principal("user:test", ws, ESCALATE);

    // An unknown subject: the same opaque refusal `case.assign` gives.
    let err = escalate(
        &node,
        &caller,
        ws,
        json!({ "case_id": case_id, "to": "user:nobody", "ts": T0 }),
    )
    .await
    .expect_err("unknown subject refused");
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("not a member")),
        "{err:?}"
    );

    // A real member with no address anywhere: refused, and NOT reassigned.
    let err = escalate(
        &node,
        &caller,
        ws,
        json!({ "case_id": case_id, "to": "user:quiet", "ts": T0 }),
    )
    .await
    .expect_err("no address refused");
    assert!(
        matches!(err, ToolError::BadInput(ref m) if m.contains("no email address for user:quiet")),
        "{err:?}"
    );

    // Without the ask grant: denied. With it but without the triage grant: denied before any write.
    for caps in [&[WORKFLOW][..], &[SEND][..]] {
        let err = escalate(
            &node,
            &principal("user:test", ws, caps),
            ws,
            json!({ "case_id": case_id, "to": "user:priya", "ts": T0 }),
        )
        .await
        .expect_err("denied");
        assert!(matches!(err, ToolError::Denied), "{caps:?}: {err:?}");
    }

    assert_eq!(
        owner_of(&node, ws, &case_id).await,
        before,
        "the owner is untouched"
    );
    let events = events_of(&node, &full, ws, &case_id).await;
    assert_eq!(count_kind(&events, "escalated"), 0);
    assert_eq!(count_kind(&events, "assigned"), 0);
    assert!(
        drain_recording(&node, ws).await.sends().is_empty(),
        "nothing mailed"
    );
}

/// Entity-scoped data: a case outside the caller's entities reads exactly like a missing one.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_out_of_scope_case_reads_as_missing() {
    let node = Arc::new(Node::boot().await.expect("node boots"));
    let ws = "nube";
    seed_people(&node, ws).await;
    let full = principal("user:test", ws, ALL);
    let case_id = seed_case(&node, &full, ws, "escalate-scope").await;

    // An enforced row policy narrowing cases by `site`, with a grant-only source: a caller holding no
    // entity grant sees no case at all (the seeded one has no site — fail closed).
    let admin = principal("user:admin", ws, &["mcp:federation.row_policy_set:call"]);
    call(
        &node,
        &admin,
        ws,
        "federation.row_policy_set",
        json!({
            "source": "probe", "enforce": true, "entity_table": "site",
            "scope_sources": ["grant"], "insight_tag": "site",
            "policy": { "tables": {}, "entity_key": {} },
        }),
    )
    .await
    .expect("row policy set");

    let caller = principal("user:test", ws, ESCALATE);
    let hidden = escalate(
        &node,
        &caller,
        ws,
        json!({ "case_id": case_id, "to": "user:priya", "ts": T0 }),
    )
    .await
    .expect_err("out of scope");
    let missing = escalate(
        &node,
        &caller,
        ws,
        json!({ "case_id": "no-such", "to": "user:priya", "ts": T0 }),
    )
    .await
    .expect_err("missing");
    let text = |e: ToolError| match e {
        ToolError::BadInput(m) => m.replace(&case_id, "<id>").replace("no-such", "<id>"),
        other => panic!("expected bad input, got {other:?}"),
    };
    assert_eq!(
        text(hidden),
        text(missing),
        "out of scope must read as missing"
    );
    assert!(drain_recording(&node, ws).await.sends().is_empty());
}
