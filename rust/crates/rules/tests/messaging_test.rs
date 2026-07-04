//! Slice 1 — the `inbox`/`outbox` rhai handles + the per-run [`WriteMeter`] (rules-messaging-scope).
//! These run the REAL engine path (sandbox + governors + scope), routing the handles through a
//! recording messaging seam that stands in for the host's `HostMessagingSeam` (the sanctioned trait
//! boundary — the real `call_tool`/caps chokepoint is proven in the host integration tests). We assert:
//!   - each handle dispatches the right MCP tool with the caller's JSON;
//!   - reads (`inbox.list`/`outbox.status`) are uncharged; writes are charged;
//!   - a write loop past the per-run cap aborts with a budget error (the DoS bound);
//!   - ids are deterministic (`now` + counter) so a re-run upserts (no wall-clock/random);
//!   - a denied verb surfaces an OPAQUE error with NO partial write (the caller-gated regression).

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

#[test]
fn inbox_record_dispatches_the_verb_with_caller_json() {
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(
        &eng,
        r#"inbox.record(#{ channel: "ops", id: "cooler-1", body: "hot" });"#,
        1234,
    )
    .unwrap();
    let calls = m.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "inbox.record");
    assert_eq!(calls[0].1["channel"], "ops");
    assert_eq!(calls[0].1["id"], "cooler-1");
    assert_eq!(calls[0].1["body"], "hot");
    // The run's logical clock is threaded as `ts` (no wall-clock).
    assert_eq!(calls[0].1["ts"], 1234);
}

#[test]
fn inbox_resolve_takes_a_string_verdict() {
    // The decision is a bare verdict string (the `Decision` enum) — a valid one dispatches with the
    // verdict + item_id; an invalid one is rejected at the handle before any dispatch.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(&eng, r#"inbox.resolve("check-me", "approved");"#, 5).unwrap();
    let calls = m.calls();
    let res = calls.iter().find(|(t, _)| t == "inbox.resolve").unwrap();
    assert_eq!(res.1["item_id"], "check-me");
    assert_eq!(res.1["decision"], "approved");

    // A bogus verdict is an author error, no dispatch.
    let m2 = Arc::new(RecordingMessaging::new());
    let err = run(&engine(m2.clone(), 32), r#"inbox.resolve("x", "ack");"#, 1).unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    assert_eq!(m2.count("inbox.resolve"), 0);
}

#[test]
fn outbox_enqueue_and_status_dispatch() {
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(
        &eng,
        r#"
        outbox.enqueue(#{ id: "p1", target: "notify", action: "page", payload: #{ level: "crit" } });
        outbox.status("p1");
        "#,
        7,
    )
    .unwrap();
    assert_eq!(m.count("outbox.enqueue"), 1);
    assert_eq!(m.count("outbox.status"), 1);
    let calls = m.calls();
    let enq = calls.iter().find(|(t, _)| t == "outbox.enqueue").unwrap();
    assert_eq!(enq.1["target"], "notify");
    assert_eq!(enq.1["action"], "page");
    assert_eq!(enq.1["payload"]["level"], "crit");
}

#[test]
fn reads_are_uncharged_writes_are_charged() {
    // max_writes = 2. A body that reads many times then writes exactly twice succeeds; a third write
    // trips. Reads never count.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 2);
    // 5 reads + 2 writes → OK (reads free).
    run(
        &eng,
        r#"
        inbox.list("ops"); inbox.list("ops"); inbox.list("ops");
        outbox.status(""); outbox.status("");
        inbox.record(#{ channel: "ops", body: "a" });
        outbox.enqueue(#{ target: "notify", action: "page" });
        "#,
        1,
    )
    .unwrap();
    assert_eq!(m.count("inbox.list"), 3);
    assert_eq!(m.count("outbox.status"), 2);
}

#[test]
fn write_meter_bounds_a_dos_loop() {
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 3); // cap = 3 writes/run
    let err = run(
        &eng,
        r#"for i in 0..1000 { outbox.enqueue(#{ target: "notify", action: "page" }); }"#,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    // Exactly the budget's worth of writes reached the seam — the loop aborted at the cap, not after
    // 1000. A rejected charge is not counted, so no extra dispatch leaks.
    assert_eq!(m.count("outbox.enqueue"), 3);
}

#[test]
fn ids_are_deterministic_across_a_rerun() {
    // Two runs with the same `now` and the same body derive the SAME ids → a re-run upserts.
    let body = r#"
        inbox.record(#{ channel: "ops", body: "a" });
        outbox.enqueue(#{ target: "notify", action: "page" });
    "#;
    let m1 = Arc::new(RecordingMessaging::new());
    run(&engine(m1.clone(), 32), body, 555).unwrap();
    let m2 = Arc::new(RecordingMessaging::new());
    run(&engine(m2.clone(), 32), body, 555).unwrap();

    let id1 = m1.calls()[0].1["id"].as_str().unwrap().to_string();
    let id2 = m2.calls()[0].1["id"].as_str().unwrap().to_string();
    assert_eq!(id1, id2, "same now+counter ⇒ same id");
    // The id carries the logical clock, not a wall-clock timestamp.
    assert!(id1.contains("555"), "id {id1} embeds the logical now");

    // And a DIFFERENT now yields a different id (so distinct runs don't collide).
    let m3 = Arc::new(RecordingMessaging::new());
    run(&engine(m3.clone(), 32), body, 556).unwrap();
    assert_ne!(id1, m3.calls()[0].1["id"].as_str().unwrap());
}

#[test]
fn denied_verb_is_opaque_with_no_partial_write() {
    // The caller lacks `outbox.enqueue` (the seam denies it). A rule that records an inbox item then
    // enqueues surfaces an OPAQUE error at the enqueue; the enqueue produced NO write (deny before the
    // dispatch is recorded). The earlier inbox write DID commit (a rule is not a transaction — the
    // deny test asserts no partial write WITHIN the denied verb, not across the body).
    let m = Arc::new(RecordingMessaging::deny(&["outbox.enqueue"]));
    let eng = engine(m.clone(), 32);
    let err = run(
        &eng,
        r#"
        inbox.record(#{ channel: "ops", body: "a" });
        outbox.enqueue(#{ target: "notify", action: "page" });
        "#,
        1,
    )
    .unwrap_err();
    // Opaque: the surfaced error carries no plane/cap detail beyond "denied".
    match err {
        RuleError::Eval(m) => assert!(m.contains("denied"), "expected opaque deny, got {m}"),
        other => panic!("expected an eval error carrying the opaque deny, got {other:?}"),
    }
    // The inbox write committed; the denied enqueue reached the seam but recorded NO write.
    assert_eq!(m.count("inbox.record"), 1);
    assert_eq!(m.count("outbox.enqueue"), 0);
}

// --- Slice 3 — the `channel` rhai handle + the worker-kind fence -----------------------------------

#[test]
fn channel_post_history_edit_delete_list_dispatch() {
    // The full channel surface round-trips through the handle → the right MCP tool + caller JSON.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(
        &eng,
        r#"
        channel.post("ops", #{ id: "m1", body: "hello" });
        channel.history("ops", 5);
        channel.edit("ops", "m1", #{ body: "edited" });
        channel.delete("ops", "m1");
        channel.list();
        "#,
        99,
    )
    .unwrap();
    let calls = m.calls();
    let post = calls.iter().find(|(t, _)| t == "channel.post").unwrap();
    assert_eq!(post.1["cid"], "ops");
    assert_eq!(post.1["id"], "m1");
    assert_eq!(post.1["body"], "hello");
    assert_eq!(post.1["ts"], 99);
    let hist = calls.iter().find(|(t, _)| t == "channel.history").unwrap();
    assert_eq!(hist.1["cid"], "ops");
    assert_eq!(hist.1["n"], 5);
    let edit = calls.iter().find(|(t, _)| t == "channel.edit").unwrap();
    assert_eq!(edit.1["id"], "m1");
    assert_eq!(edit.1["body"], "edited");
    let del = calls.iter().find(|(t, _)| t == "channel.delete").unwrap();
    assert_eq!(del.1["id"], "m1");
    assert_eq!(m.count("channel.list"), 1);
}

#[test]
fn channel_reads_are_uncharged_writes_are_charged() {
    // cap = 1. Many reads (history/list) + exactly one write (post) succeed; reads never charge.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 1);
    run(
        &eng,
        r#"
        channel.history("ops", 3); channel.history("ops"); channel.list(); channel.list();
        channel.post("ops", #{ body: "one" });
        "#,
        1,
    )
    .unwrap();
    assert_eq!(m.count("channel.history"), 2);
    assert_eq!(m.count("channel.list"), 2);
    assert_eq!(m.count("channel.post"), 1);
}

#[test]
fn channel_post_loop_is_bounded_by_the_write_meter() {
    // A DoS loop posting to a channel aborts at the per-run cap; exactly the budget reaches the seam.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 3);
    let err = run(
        &eng,
        r#"for i in 0..1000 { channel.post("ops", #{ body: `msg ${i}` }); }"#,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    assert_eq!(m.count("channel.post"), 3);
}

#[test]
fn channel_post_rejects_agent_kind_before_any_write() {
    // The fence: a `kind:"agent"` post is rejected AT THE HANDLE with an author error — a rule cannot
    // spawn a run — and NO write reaches the seam AND the write meter is not charged (the reject is
    // before `charge()`).
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    let err = run(
        &eng,
        r#"channel.post("ops", #{ kind: "agent", goal: "summarize" });"#,
        1,
    )
    .unwrap_err();
    match err {
        RuleError::Eval(msg) => {
            assert!(msg.contains("cannot spawn a run"), "got {msg}");
            assert!(
                msg.contains("flow"),
                "author is pointed at a flow, got {msg}"
            );
        }
        other => panic!("expected the fence author error, got {other:?}"),
    }
    // The fence fired BEFORE the seam — no channel write at all.
    assert_eq!(m.count("channel.post"), 0);
}

#[test]
fn channel_post_rejects_query_kind() {
    // The same fence for `kind:"query"` — a rule cannot kick an inline query worker either.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    let err = run(
        &eng,
        r#"channel.post("ops", #{ kind: "query", source: "s", sql: "SELECT 1" });"#,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    assert_eq!(m.count("channel.post"), 0);
}

#[test]
fn channel_post_text_kind_passes_the_fence() {
    // `kind:"text"` (an explicit plain chat) is NOT a worker kind — it posts, body is the raw text.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32);
    run(
        &eng,
        r#"channel.post("ops", #{ kind: "text", body: "just chat" });"#,
        1,
    )
    .unwrap();
    let calls = m.calls();
    let post = calls.iter().find(|(t, _)| t == "channel.post").unwrap();
    assert_eq!(post.1["body"], "just chat");
}

#[test]
fn channel_post_denied_is_opaque_with_no_partial_write() {
    // The caller lacks the channel `Pub` cap (the seam denies `channel.post`). The rule surfaces an
    // OPAQUE "denied" and the denied post produced NO write.
    let m = Arc::new(RecordingMessaging::deny(&["channel.post"]));
    let eng = engine(m.clone(), 32);
    let err = run(&eng, r#"channel.post("ops", #{ body: "hi" });"#, 1).unwrap_err();
    match err {
        RuleError::Eval(msg) => assert!(msg.contains("denied"), "expected opaque deny, got {msg}"),
        other => panic!("expected the opaque deny, got {other:?}"),
    }
    assert_eq!(m.count("channel.post"), 0);
}

#[test]
fn channel_post_ids_are_deterministic_across_a_rerun() {
    // Same `now` + body ⇒ same derived post id (a re-run upserts, no duplicate); the id embeds the
    // logical clock, not a wall-clock/random value.
    let body = r#"channel.post("ops", #{ body: "a" });"#;
    let m1 = Arc::new(RecordingMessaging::new());
    run(&engine(m1.clone(), 32), body, 555).unwrap();
    let m2 = Arc::new(RecordingMessaging::new());
    run(&engine(m2.clone(), 32), body, 555).unwrap();
    let id1 = m1.calls()[0].1["id"].as_str().unwrap().to_string();
    let id2 = m2.calls()[0].1["id"].as_str().unwrap().to_string();
    assert_eq!(id1, id2, "same now+counter ⇒ same id");
    assert!(id1.contains("555"), "id {id1} embeds the logical now");
}
