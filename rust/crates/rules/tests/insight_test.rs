//! The `insight` rhai handle — the rule producer door onto the insight plane (rule-raises-insight-scope).
//! These run the REAL engine path (sandbox + governors + scope + write meter), routing the handle
//! through the sanctioned `RecordingMessaging` seam that stands in for the host's `HostMessagingSeam`
//! (the real `call_tool`/caps chokepoint + the real `insight.raise` verb + a `mem://` store are proven
//! end-to-end in `host/tests/rules_test.rs`). We assert here:
//!   - `raise`/`ack`/`close` dispatch the right MCP verb (`close` → `insight.resolve`) with caller JSON;
//!   - `raise` returns the outcome id; `ts` is the run's logical clock; a missing `origin` defaults to
//!     `{ kind:"rule", ref:<rule name> }` (the run's provenance);
//!   - writes are charged, a raise loop past `max_writes` trips the governor (the DoS bound);
//!   - a `route:false` run no-ops every method (no dispatch, no charge, an honest skip log line);
//!   - a denied verb surfaces an OPAQUE error with NO dispatch (the caller-gated regression).

mod support;

use std::collections::HashSet;
use std::sync::Arc;

use lb_rules::{AiLimits, Rule, RuleEngine, RuleError, RuleLimits, RuleRun};
use support::{RecordingData, RecordingMessaging, ScriptedAi};

fn engine(messaging: Arc<RecordingMessaging>, max_writes: u32, route: bool) -> RuleEngine {
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
    .with_route(route)
}

/// Run a rule named `rule_name` (its name is the origin ref the handle stamps).
fn run_named(
    eng: &RuleEngine,
    rule_name: &str,
    body: &str,
    now: u64,
) -> Result<(lb_rules::RuleOutput, Vec<lb_rules::LogLine>), RuleError> {
    let rule = Rule {
        workspace: "acme".into(),
        name: rule_name.into(),
        body: body.into(),
        params: vec![],
    };
    let mut rr = RuleRun::new(
        "acme".into(),
        Arc::new(HashSet::new()),
        rhai::Map::new(),
        now,
    );
    let out = eng.run(&rule, &mut rr)?;
    Ok((out, rr.log))
}

fn run(eng: &RuleEngine, body: &str, now: u64) -> Result<lb_rules::RuleOutput, RuleError> {
    run_named(eng, "adhoc", body, now).map(|(o, _)| o)
}

#[test]
fn raise_dispatches_the_verb_and_returns_the_id() {
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32, true);
    // The body binds the returned id and re-uses it — proving `raise` yields a usable String.
    run(
        &eng,
        r#"
        let id = insight.raise(#{
            dedup_key: "cooler-temp-high",
            severity: "warning",
            title: "Cooler temp high",
            body: #{ series: "cooler.temp", value: 9.1 },
            tags: #{ area: "hvac" },
        });
        emit(#{ level: "info", insight_id: id });
        "#,
        1234,
    )
    .unwrap();
    let calls = m.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "insight.raise");
    let input = &calls[0].1;
    assert_eq!(input["dedup_key"], "cooler-temp-high");
    assert_eq!(input["severity"], "warning");
    assert_eq!(input["title"], "Cooler temp high");
    assert_eq!(input["body"]["value"], 9.1);
    assert_eq!(input["tags"]["area"], "hvac");
    // The run's logical clock is injected as `ts` (no wall-clock).
    assert_eq!(input["ts"], 1234);
}

#[test]
fn raise_defaults_origin_to_the_rules_provenance() {
    // A raise omitting `origin` gets `{ kind:"rule", ref:<rule name> }` synthesized by the cage.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32, true);
    run_named(
        &eng,
        "rule:acme:cooler-watch",
        r#"insight.raise(#{ dedup_key: "k", severity: "critical", title: "t" });"#,
        1,
    )
    .unwrap();
    let calls = m.calls();
    assert_eq!(calls[0].1["origin"]["kind"], "rule");
    assert_eq!(calls[0].1["origin"]["ref"], "rule:acme:cooler-watch");
}

#[test]
fn raise_keeps_an_author_supplied_origin() {
    // If the author DOES pass an origin, the cage doesn't clobber it.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32, true);
    run(
        &eng,
        r#"insight.raise(#{ dedup_key: "k", severity: "info", title: "t",
                            origin: #{ kind: "rule", ref: "custom-ref" } });"#,
        1,
    )
    .unwrap();
    assert_eq!(m.calls()[0].1["origin"]["ref"], "custom-ref");
}

#[test]
fn raise_missing_required_field_is_author_feedback_no_dispatch() {
    // A missing `dedup_key`/`severity`/`title` is BadInput author feedback surfaced verbatim — NOT an
    // opaque deny — and reaches no dispatch (and no charge).
    for (body, missing) in [
        (
            r#"insight.raise(#{ severity: "info", title: "t" });"#,
            "dedup_key",
        ),
        (
            r#"insight.raise(#{ dedup_key: "k", title: "t" });"#,
            "severity",
        ),
        (
            r#"insight.raise(#{ dedup_key: "k", severity: "info" });"#,
            "title",
        ),
    ] {
        let m = Arc::new(RecordingMessaging::new());
        let err = run(&engine(m.clone(), 32, true), body, 1).unwrap_err();
        match err {
            RuleError::Eval(msg) => assert!(msg.contains(missing), "expected `{missing}` in {msg}"),
            other => panic!("expected author feedback, got {other:?}"),
        }
        assert_eq!(m.count("insight.raise"), 0);
    }
}

#[test]
fn ack_and_close_dispatch_the_right_verbs() {
    // `ack` → `insight.ack`; `close` → `insight.resolve` (the author-facing name maps to the verb).
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32, true);
    run(
        &eng,
        r#"
        insight.ack("insight-1");
        insight.close("insight-1");
        insight.close("insight-2", "temp cleared");
        "#,
        77,
    )
    .unwrap();
    let calls = m.calls();
    let ack = calls.iter().find(|(t, _)| t == "insight.ack").unwrap();
    assert_eq!(ack.1["id"], "insight-1");
    assert_eq!(ack.1["ts"], 77);
    // Two resolves — `close` maps to `insight.resolve`; the 2-arg form carries the note.
    let resolves: Vec<_> = calls
        .iter()
        .filter(|(t, _)| t == "insight.resolve")
        .collect();
    assert_eq!(resolves.len(), 2);
    assert_eq!(resolves[0].1["id"], "insight-1");
    assert!(resolves[0].1.get("note").is_none());
    assert_eq!(resolves[1].1["id"], "insight-2");
    assert_eq!(resolves[1].1["note"], "temp cleared");
    assert_eq!(
        m.count("insight.close"),
        0,
        "there is no `insight.close` verb — close maps to resolve"
    );
}

#[test]
fn writes_are_charged_a_raise_loop_trips_the_meter() {
    // cap = 3. A loop raising past the cap aborts at the budget (the DoS bound) — exactly the budget's
    // worth reaches the seam, no more (a rejected charge is not counted).
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 3, true);
    let err = run(
        &eng,
        r#"for i in 0..1000 { insight.raise(#{ dedup_key: `k${i}`, severity: "info", title: "t" }); }"#,
        1,
    )
    .unwrap_err();
    assert!(matches!(err, RuleError::Eval(_)), "got {err:?}");
    assert_eq!(m.count("insight.raise"), 3);
}

#[test]
fn route_false_no_ops_every_method_and_logs_the_skip() {
    // A `route:false` (read-only panel) run: raise/ack/close write NOTHING, charge NOTHING, and each
    // records an honest skip log line. `raise` still returns an echoed id so the body doesn't error.
    let m = Arc::new(RecordingMessaging::new());
    let eng = engine(m.clone(), 32, false);
    let (_out, log) = run_named(
        &eng,
        "adhoc",
        r#"
        let id = insight.raise(#{ dedup_key: "k", severity: "warning", title: "t" });
        insight.ack(id);
        insight.close(id);
        emit(#{ level: "info", echoed: id });
        "#,
        1,
    )
    .unwrap();
    // NO dispatch reached the seam at all.
    assert_eq!(
        m.calls().len(),
        0,
        "route:false suppresses every insight write"
    );
    // Three honest skip lines (raise/ack/close), each naming the read-only reason.
    let skips: Vec<_> = log
        .iter()
        .filter(|l| l.message.contains("skipped: read-only panel run"))
        .collect();
    assert_eq!(
        skips.len(),
        3,
        "one skip line per suppressed call, got {:?}",
        log
    );
    assert!(skips.iter().any(|l| l.message.contains("insight.raise")));
    assert!(skips.iter().any(|l| l.message.contains("insight.ack")));
    assert!(skips.iter().any(|l| l.message.contains("insight.close")));
}

#[test]
fn route_true_writes_where_route_false_did_not() {
    // The contrast: the SAME body at route:true DOES dispatch (proving the flag is what suppressed).
    let body = r#"
        let id = insight.raise(#{ dedup_key: "k", severity: "warning", title: "t" });
        insight.ack(id);
        insight.close(id);
    "#;
    let routed = Arc::new(RecordingMessaging::new());
    run(&engine(routed.clone(), 32, true), body, 1).unwrap();
    assert_eq!(routed.count("insight.raise"), 1);
    assert_eq!(routed.count("insight.ack"), 1);
    assert_eq!(routed.count("insight.resolve"), 1);

    let panel = Arc::new(RecordingMessaging::new());
    run(&engine(panel.clone(), 32, false), body, 1).unwrap();
    assert_eq!(panel.calls().len(), 0);
}

#[test]
fn denied_raise_is_opaque_with_no_partial_write() {
    // The caller lacks `mcp:insight.raise:call` (the seam denies it). The rule surfaces an OPAQUE
    // "denied" and NO write reached the store for the denied verb.
    let m = Arc::new(RecordingMessaging::deny(&["insight.raise"]));
    let eng = engine(m.clone(), 32, true);
    let err = run(
        &eng,
        r#"insight.raise(#{ dedup_key: "k", severity: "critical", title: "t" });"#,
        1,
    )
    .unwrap_err();
    match err {
        RuleError::Eval(msg) => assert!(msg.contains("denied"), "expected opaque deny, got {msg}"),
        other => panic!("expected the opaque deny, got {other:?}"),
    }
    assert_eq!(m.count("insight.raise"), 0);
}

#[test]
fn denied_close_after_a_landed_raise_is_opaque() {
    // A rule that raises OK (the raise commits) but lacks `mcp:insight.resolve:call` is denied mid-run
    // at `close` — opaquely, after the raise already landed (a rule is not a transaction).
    let m = Arc::new(RecordingMessaging::deny(&["insight.resolve"]));
    let eng = engine(m.clone(), 32, true);
    let err = run(
        &eng,
        r#"
        let id = insight.raise(#{ dedup_key: "k", severity: "warning", title: "t" });
        insight.close(id);
        "#,
        1,
    )
    .unwrap_err();
    match err {
        RuleError::Eval(msg) => assert!(msg.contains("denied"), "expected opaque deny, got {msg}"),
        other => panic!("expected the opaque deny, got {other:?}"),
    }
    // The raise committed; the denied resolve reached the seam but recorded NO write.
    assert_eq!(m.count("insight.raise"), 1);
    assert_eq!(m.count("insight.resolve"), 0);
}

#[test]
fn ts_is_deterministic_across_a_rerun() {
    // Two runs at the same `now` inject the same `ts` → the verb's `(ws, dedup_key)` dedup upserts
    // idempotently (no client-side id generation, no wall-clock).
    let body = r#"insight.raise(#{ dedup_key: "k", severity: "info", title: "t" });"#;
    let m1 = Arc::new(RecordingMessaging::new());
    run(&engine(m1.clone(), 32, true), body, 555).unwrap();
    let m2 = Arc::new(RecordingMessaging::new());
    run(&engine(m2.clone(), 32, true), body, 555).unwrap();
    assert_eq!(m1.calls()[0].1["ts"], 555);
    assert_eq!(m2.calls()[0].1["ts"], 555);
    // A different now threads a different ts (distinct logical runs).
    let m3 = Arc::new(RecordingMessaging::new());
    run(&engine(m3.clone(), 32, true), body, 556).unwrap();
    assert_eq!(m3.calls()[0].1["ts"], 556);
}

#[test]
fn catalog_carries_the_insight_family() {
    // The three rows are present under the new `insight` family (the integrity tests in `catalog.rs`
    // guard the known-set; this asserts the rows a rule author sees via `rules.help`).
    use lb_rules::catalog::CATALOG;
    let insight: Vec<_> = CATALOG.iter().filter(|e| e.family == "insight").collect();
    let names: HashSet<&str> = insight.iter().map(|e| e.name).collect();
    assert!(names.contains("insight.raise"));
    assert!(names.contains("insight.ack"));
    assert!(names.contains("insight.close"));
}
