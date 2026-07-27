//! `series.producer.health` at the MCP surface (series-observability scope, slice D) — the verb that
//! asks the things WRITING a series what they say about their own ingest.
//!
//! This file holds the CONTRACT. The two mandatory categories (capability-deny both directions,
//! workspace isolation) live in `series_producer_health_caps_test.rs`, and the shared real-infra
//! fixture in `support/producer_health.rs` — the same split as
//! `series_normalize_test.rs` / `series_normalize_caps_test.rs`.
//!
//! Everything is real (testing §0): a real `Node::boot()`, a real store, real samples through
//! `lb_ingest::write` + `drain_workspace`, the real registry, and the real `call_tool` dispatch
//! chokepoint — so the new arm in `tool_call.rs`, the capability gates and the re-entrant extension
//! call are all the production path.
//!
//! **Load-bearing:** every way of not-knowing must stay distinguishable from data AND from each
//! other. `not-an-extension`, `not-reported`, `denied` and `error` are four different answers, and a
//! test that only checked "the row is not Reported" would pass against a verb that collapsed them —
//! which is the precise failure (a refusal that looks like a healthy blank) this whole scope exists
//! to kill. So each is asserted by name.

use std::sync::Arc;

use lb_host::{Node, PRODUCER_HEALTH_TOOL};
use serde_json::Value;

#[path = "support/producer_health.rs"]
mod support;
use support::{admin, health, register_reporter, register_silent, row, seed, SERIES};

// -------------------------------------------------------- the four ways of not knowing, named ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_that_is_not_an_extension_says_so_and_nothing_is_wrong() {
    // Most series in a workspace look like this: a human, a flow, a webhook, the hand-write form.
    // The strip must be ABSENT for them, not broken and not empty-looking.
    let node = Arc::new(Node::boot().await.unwrap());
    seed(&node, "acme", "user:ada/gw-alpha", 1).await;

    let out = health(&node, &admin("acme", &[]), "acme").await.unwrap();
    let r = row(&out, "user:ada/gw-alpha");
    assert_eq!(r["state"], "not-an-extension");
    assert_eq!(r["ext"], Value::Null);
    assert_eq!(r["report"], Value::Null);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_extension_that_declares_no_health_tool_reports_nothing_and_is_not_an_error() {
    let node = Arc::new(Node::boot().await.unwrap());
    register_silent(&node, "quiet-ext");
    seed(&node, "acme", "ext:quiet-ext/net-1", 1).await;

    let out = health(&node, &admin("acme", &[]), "acme").await.unwrap();
    let r = row(&out, "ext:quiet-ext/net-1");
    assert_eq!(r["state"], "not-reported");
    // The ext IS identified — we know who is silent, which is itself useful.
    assert_eq!(r["ext"], "quiet-ext");
    assert_eq!(r["message"], Value::Null, "silence is not an error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_refusal_names_the_missing_grant_and_never_looks_like_silence() {
    // THE highest-value assertion in the file. The caller may run the verb but holds no
    // `mcp:<ext>.ingest.health:call`, so the per-extension call is refused. That MUST NOT collapse
    // into `not-reported` — "you may not ask" and "it has nothing to say" are different facts, and
    // conflating them is the silent degrade the scope was written to end.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:demo-probe/net-1", 1).await;

    let out = health(&node, &admin("acme", &[]), "acme").await.unwrap();
    let r = row(&out, "ext:demo-probe/net-1");
    assert_eq!(r["state"], "denied");
    assert_ne!(r["state"], "not-reported");
    assert_eq!(
        r["missing_cap"], "mcp:demo-probe.ingest.health:call",
        "the panel must be able to name the grant to ask for"
    );
    assert_eq!(r["report"], Value::Null, "a refusal carries no data");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reply_in_a_shape_we_cannot_read_is_an_error_not_a_plausible_blank() {
    let node = Arc::new(Node::boot().await.unwrap());
    // `last_write_ms` is a number; a string there is a contract bug on the producer's side.
    register_reporter(
        &node,
        "broken-ext",
        r#"{"last_write_ms":"yesterday"}"#,
        false,
    );
    seed(&node, "acme", "ext:broken-ext/net-1", 1).await;

    let p = admin("acme", &["mcp:broken-ext.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let r = row(&out, "ext:broken-ext/net-1");
    assert_eq!(r["state"], "error");
    assert_eq!(r["report"], Value::Null);
    assert!(
        r["message"]
            .as_str()
            .expect("an error carries its text")
            .contains(PRODUCER_HEALTH_TOOL),
        "a broken producer must be diagnosable, not merely blank: {r}"
    );
}

// ------------------------------------------------------------------------ the reporting path ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_declaring_extension_is_asked_and_its_report_is_carried_verbatim() {
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(
        &node,
        "demo-probe",
        r#"{"state":"reconnecting","last_write_ms":1700000000123,"last_accepted":42,
            "details":[{"label":"consecutive timeouts","value":"11"}]}"#,
        false,
    );
    seed(&node, "acme", "ext:demo-probe/net-1", 1).await;

    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let r = row(&out, "ext:demo-probe/net-1");

    assert_eq!(r["state"], "reported");
    assert_eq!(r["ext"], "demo-probe");
    let rep = &r["report"];
    assert_eq!(rep["state"], "reconnecting");
    assert_eq!(rep["last_write_ms"], 1_700_000_000_123u64);
    assert_eq!(rep["last_accepted"], 42);
    // The domain-specific fact rides through untouched — the host models no `consecutive_timeouts`
    // field, because doing so would encode "a producer is a polling device" into the core.
    assert_eq!(rep["details"][0]["label"], "consecutive timeouts");
    assert_eq!(rep["details"][0]["value"], "11");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_producer_is_handed_its_own_stream_id_not_the_rooted_form() {
    // A producer never saw the `ext:<id>/` root the host stamped on. Handing back the rooted string
    // would make an extension feeding several streams unable to tell which one is being asked about
    // — it would have to guess, or answer for all of them.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, true);
    seed(&node, "acme", "ext:demo-probe/net-7@42", 1).await;

    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let details = &row(&out, "ext:demo-probe/net-7@42")["report"]["details"];

    assert_eq!(details[0]["value"], "net-7@42", "the leaf it declared");
    assert_eq!(details[1]["value"], SERIES, "the series being asked about");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_missing_report_field_stays_absent_and_is_never_defaulted_to_zero() {
    // A `0` where the producer said nothing is a fabricated measurement — it reads as "accepted
    // none", which is a different and possibly false claim from "did not say".
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "terse-ext", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:terse-ext/net-1", 1).await;

    let p = admin("acme", &["mcp:terse-ext.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let rep = &row(&out, "ext:terse-ext/net-1")["report"];

    assert_eq!(rep["state"], "connected");
    assert_eq!(rep["last_write_ms"], Value::Null);
    assert_eq!(rep["last_accepted"], Value::Null);
    assert_ne!(rep["last_accepted"], 0);
    assert_eq!(
        rep["details"]
            .as_array()
            .expect("details is an array")
            .len(),
        0
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn one_broken_producer_does_not_blank_the_healthy_one_beside_it() {
    // A series is written by a SET of producers. If one extension is down, the strip must still
    // report the other — a whole-read failure would hide working data behind a broken neighbour.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(
        &node,
        "good-ext",
        r#"{"state":"connected","last_accepted":7}"#,
        false,
    );
    register_reporter(&node, "bad-ext", r#"not json at all"#, false);
    seed(&node, "acme", "ext:good-ext/net-1", 1).await;
    seed(&node, "acme", "ext:bad-ext/net-1", 2).await;
    seed(&node, "acme", "user:ada/manual", 3).await;

    let p = admin(
        "acme",
        &[
            "mcp:good-ext.ingest.health:call",
            "mcp:bad-ext.ingest.health:call",
        ],
    );
    let out = health(&node, &p, "acme").await.unwrap();

    assert_eq!(row(&out, "ext:good-ext/net-1")["state"], "reported");
    assert_eq!(
        row(&out, "ext:good-ext/net-1")["report"]["last_accepted"],
        7
    );
    assert_eq!(row(&out, "ext:bad-ext/net-1")["state"], "error");
    assert_eq!(row(&out, "user:ada/manual")["state"], "not-an-extension");
    assert_eq!(out["producers"].as_array().unwrap().len(), 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_series_with_no_samples_is_an_empty_list_not_an_error() {
    let node = Arc::new(Node::boot().await.unwrap());
    let out = health(&node, &admin("acme", &[]), "acme").await.unwrap();
    assert_eq!(out["series"], SERIES);
    assert_eq!(out["producers"].as_array().expect("an array").len(), 0);
}
