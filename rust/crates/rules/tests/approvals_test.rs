//! `inbox.request_approval` — the rule verb that raises a `needs:approval` item AND stages the gated
//! `on_approve` effect held (rules-approvals-scope). These run the REAL engine path (sandbox +
//! governors + the shared [`WriteMeter`]) through a recording messaging seam standing in for the
//! host's `HostMessagingSeam` (the real `call_tool`/caps/reactor path is proven in the host
//! integration test `approval_release_test.rs`). We assert:
//!   - the verb makes TWO writes: `outbox.enqueue_held` FIRST (partial-failure contract), then
//!     `inbox.record` of the `needs:approval`-tagged item — with the caller's JSON;
//!   - both are charged against the shared per-run budget (a tight cap trips it);
//!   - the item id is deterministic + returned so a follow-on run can address it;
//!   - a deny on the effect stage is OPAQUE and leaves NO partial write (the item is never recorded —
//!     the effect-first order is what guarantees an item never dangles without its effect).

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, Rule, RuleEngine, RuleError, RuleLimits, RuleRun};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn engine(messaging: Arc<RecordingMessaging>, max_writes: u32) -> RuleEngine {
    let data = Arc::new(RecordingData::platform(
        &["series"],
        lb_rules::GridJson {
            columns: vec![],
            rows: vec![],
        },
    ));
    let ai = Arc::new(ScriptedAi {
        completion: "ok".into(),
        tokens: 1,
        proposed_sql: "SELECT 1 AS v".into(),
    });
    RuleEngine::new(
        data,
        ai,
        messaging,
        RuleLimits::default(),
        AiLimits::default(),
        max_writes,
    )
}

fn run(eng: &RuleEngine, body: &str, now: u64) -> Result<lb_rules::RuleOutput, RuleError> {
    let rule = Rule {
        workspace: "acme".into(),
        name: "adhoc".into(),
        body: body.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new(
        "acme".into(),
        Arc::new(HashSet::new()),
        rhai::Map::new(),
        now,
    );
    eng.run(&rule, &mut rr)
}

const REQUEST_APPROVAL: &str = r#"
    inbox.request_approval(#{
        id: "refund-42",
        channel: "ops",
        body: "Refund proposed",
        route: "team:managers",
        on_approve: #{ target: "email", action: "send",
                       payload: #{ to: "ops@acme.io", subject: "Refund approved" } },
    });
"#;

#[test]
fn request_approval_stages_the_held_effect_first_then_records_the_tagged_item() {
    // THE HEADLINE: one verb, two writes, in the partial-failure-safe order (effect held FIRST, item
    // SECOND) — with the `needs:approval route:…` body tag the reviewer UI / release reactor parse.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(&eng, REQUEST_APPROVAL, 1000).unwrap();

    let calls = m.calls();
    assert_eq!(calls.len(), 2, "exactly two writes");

    // Effect FIRST — so a mid-verb fault never leaves a recorded item without its gated effect.
    assert_eq!(calls[0].0, "outbox.enqueue_held");
    assert_eq!(calls[0].1["item_id"], "refund-42");
    assert_eq!(calls[0].1["target"], "email");
    assert_eq!(calls[0].1["action"], "send");
    assert_eq!(calls[0].1["payload"]["to"], "ops@acme.io");
    assert_eq!(calls[0].1["ts"], 1000);

    // Item SECOND — the `needs:approval` tag + route ride the body-tag convention.
    assert_eq!(calls[1].0, "inbox.record");
    assert_eq!(calls[1].1["channel"], "ops");
    assert_eq!(calls[1].1["id"], "refund-42");
    let body = calls[1].1["body"].as_str().unwrap();
    assert!(
        body.starts_with("needs:approval route:team:managers "),
        "tagged body: {body}"
    );
    assert!(body.contains("Refund proposed"));
}

#[test]
fn request_approval_charges_two_writes_against_the_shared_budget() {
    // Both the held effect and the item are motion-producing writes — a per-run cap of 1 trips on the
    // second (the item record), proving the DoS budget counts BOTH, not just the item.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 1);
    let err = run(&eng, REQUEST_APPROVAL, 1000).unwrap_err();
    assert!(
        matches!(err, RuleError::Eval(_)),
        "the second write trips the budget: {err:?}"
    );
    // The effect (first write) got through; the item (second) was refused by the meter before dispatch.
    assert_eq!(m.count("outbox.enqueue_held"), 1);
    assert_eq!(
        m.count("inbox.record"),
        0,
        "the budgeted second write never dispatched"
    );
}

#[test]
fn a_deny_on_the_held_effect_is_opaque_with_no_item_recorded() {
    // CALLER-GATED (mandatory, no-partial-write): the caller lacks the outbox stage cap. Because the
    // effect is staged FIRST, the deny aborts the verb BEFORE the item is recorded — so a
    // `needs:approval` item never exists without its gated effect. The error is opaque ("denied").
    let m = Arc::new(RecordingMessaging::deny(&["outbox.enqueue_held"]));
    let eng = engine(m.clone(), 32);
    let err = run(&eng, REQUEST_APPROVAL, 1000).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "opaque deny: {err:?}");
    // NO write reached the store — not the effect (denied), and crucially not the item (never tried).
    assert_eq!(m.count("outbox.enqueue_held"), 0);
    assert_eq!(
        m.count("inbox.record"),
        0,
        "no dangling needs:approval item"
    );
}

#[test]
fn request_approval_returns_the_item_id_for_a_follow_on_resolve() {
    // The verb returns the item id so a later run (or the same body) can `inbox.resolve` it. A derived
    // id is deterministic (`now`+counter) when none is supplied.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    let out = run(
        &eng,
        r#"
        let id = inbox.request_approval(#{
            channel: "ops", body: "x",
            on_approve: #{ target: "email", action: "send", payload: #{} },
        });
        inbox.resolve(id, "approved");
        "#,
        1000,
    )
    .unwrap();
    let _ = out;
    let calls = m.calls();
    // enqueue_held, record, resolve — the resolve addresses the SAME derived id the request returned.
    assert_eq!(calls.len(), 3);
    let staged_id = calls[0].1["item_id"].as_str().unwrap();
    assert_eq!(calls[2].0, "inbox.resolve");
    assert_eq!(calls[2].1["item_id"], staged_id);
}
