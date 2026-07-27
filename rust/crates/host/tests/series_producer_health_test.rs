//! `series.producer.health` at the MCP surface (series-observability scope, slice D) — the verb that
//! asks the things WRITING a series what they say about their own ingest.
//!
//! Everything here is real (testing §0): a real `Node::boot()`, a real store, real samples through
//! `lb_ingest::write` + `drain_workspace`, the real registry, and the real `call_tool` dispatch
//! chokepoint — so the new arm in `tool_call.rs`, the capability gates and the re-entrant
//! extension call are all the production path. The reporting extension is a real `LocalDispatch`,
//! the SAME trait a wasm instance and a native sidecar implement and the same one
//! `routed_host_entry_test.rs` uses to prove routed dispatch; it is a real implementor at the seam,
//! not a stand-in for the host code under test.
//!
//! **What this file deliberately does NOT claim to prove:** that a real, separately-built,
//! separately-published extension answers this convention. That is proven live against `modbus` on a
//! running node and recorded in the session doc — a contract with no out-of-tree implementor is
//! exactly the thing the scope refused to ship last time.
//!
//! **Load-bearing:** every way of not-knowing must stay distinguishable from data and from each
//! other. `not-an-extension`, `not-reported`, `denied` and `error` are four different answers, and a
//! test that only checked "the row is not Reported" would pass against a verb that collapsed them —
//! which is the precise failure (a refusal that looks like a healthy blank) this whole scope exists
//! to kill. So each is asserted by name.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, drain_workspace, Node, PRODUCER_HEALTH_TOOL};
use lb_ingest::{Qos, Sample};
use lb_mcp::{ToolDescriptor, ToolError};
use lb_runtime::{CallContext, LocalDispatch, RuntimeError};
use serde_json::{json, Value};
use tokio::sync::Mutex;

const VERB: &str = "series.producer.health";
const VERB_CAP: &str = "mcp:series.producer.health:call";
const SERIES: &str = "plant-a.chiller-1.current-l1";

// ------------------------------------------------------------------------------- the harness ----

/// A real extension-side implementor of the convention. `reply` is the raw JSON body it answers
/// with, so a test can drive the well-formed path AND the malformed one through identical plumbing.
/// It echoes the arguments it was handed into `details`, which is what proves the host passes the
/// producer its OWN stream id (the leaf) rather than the rooted form it never saw.
struct HealthReporter {
    reply: String,
    echo_args: bool,
}

#[async_trait::async_trait]
impl LocalDispatch for HealthReporter {
    async fn call_tool(
        &mut self,
        _ws: &str,
        tool: &str,
        input_json: &str,
        _ctx: Option<CallContext>,
    ) -> Result<String, RuntimeError> {
        if tool != PRODUCER_HEALTH_TOOL {
            return Err(RuntimeError::Tool(format!("unknown tool: {tool}")));
        }
        if !self.echo_args {
            return Ok(self.reply.clone());
        }
        let args: Value = serde_json::from_str(input_json).unwrap_or(json!({}));
        let mut body: Value = serde_json::from_str(&self.reply).expect("reply is json");
        body["details"] = json!([
            { "label": "echo.producer", "value": args["producer"].as_str().unwrap_or("<none>") },
            { "label": "echo.series",   "value": args["series"].as_str().unwrap_or("<none>") },
        ]);
        Ok(body.to_string())
    }
}

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// Register `ext` as a real registry entry declaring the health convention.
fn register_reporter(node: &Node, ext: &str, reply: &str, echo_args: bool) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only(PRODUCER_HEALTH_TOOL)],
        Arc::new(Mutex::new(HealthReporter {
            reply: reply.to_string(),
            echo_args,
        })),
    );
}

/// Register `ext` declaring some OTHER tool — installed and reachable, but opting out of the
/// convention. This is the honest majority case and must not read as broken.
fn register_silent(node: &Node, ext: &str) {
    node.registry.register_local_dispatch(
        ext,
        vec![ToolDescriptor::name_only("network.status")],
        Arc::new(Mutex::new(HealthReporter {
            reply: "{}".into(),
            echo_args: false,
        })),
    );
}

/// Commit one sample for `producer` on `SERIES` in `ws`, through the real write → drain path.
///
/// `lb_ingest::write` is used directly (not the `ingest.write` verb) precisely because the verb
/// would ROOT the producer at the calling principal — here the stored producer string IS the input
/// under test, so it is written verbatim.
async fn seed(node: &Node, ws: &str, producer: &str, seq: u64) {
    let s = Sample {
        series: SERIES.into(),
        producer: producer.into(),
        ts: 1_700_000_000_000 + seq,
        seq,
        payload: json!({ "v": 12.5 }),
        labels: json!({}),
        qos: Qos::BestEffort,
    };
    lb_ingest::write(&node.store, ws, &[s], 0).await.unwrap();
    drain_workspace(&node.store, ws).await.unwrap();
}

async fn health(node: &Arc<Node>, p: &Principal, ws: &str) -> Result<Value, ToolError> {
    let body = call_tool(node, p, ws, VERB, &json!({ "series": SERIES }).to_string()).await?;
    Ok(serde_json::from_str(&body).expect("verb returns json"))
}

/// The one row for `producer`, or a panic naming what WAS returned — a silently-missing row is the
/// failure mode a `.find().is_none()` assertion would hide.
fn row<'a>(out: &'a Value, producer: &str) -> &'a Value {
    out["producers"]
        .as_array()
        .expect("producers is an array")
        .iter()
        .find(|r| r["producer"] == producer)
        .unwrap_or_else(|| panic!("no row for {producer} in {out}"))
}

fn admin(ws: &str, extra: &[&str]) -> Principal {
    let mut caps = vec![VERB_CAP.to_string()];
    caps.extend(extra.iter().map(|s| s.to_string()));
    let refs: Vec<&str> = caps.iter().map(String::as_str).collect();
    principal("user:ada", ws, &refs)
}

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

// -------------------------------------------------------------------- mandatory: deny, both ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_verb_cap_the_whole_read_is_refused_opaquely() {
    // Direction 1: the outer gate. A caller holding every EXTENSION health cap but not the verb's
    // own still gets nothing — the fan-out is not a way in through the side door.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:demo-probe/net-1", 1).await;

    let p = principal("user:bob", "acme", &["mcp:demo-probe.ingest.health:call"]);
    let err = health(&node, &p, "acme").await.unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn holding_the_verb_cap_grants_no_reach_into_an_extension_it_could_not_call() {
    // Direction 2: the inner gate, stated as the privilege-escalation question. This verb calls
    // extension tools on the caller's behalf; if it ran them under any authority but the caller's,
    // `mcp:series.producer.health:call` would become a universal read of every extension on the
    // node. It does not — the row is `denied` while a DIFFERENT extension the caller CAN call still
    // reports, which also proves the gate is per-extension rather than all-or-nothing.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "allowed-ext", r#"{"state":"connected"}"#, false);
    register_reporter(&node, "forbidden-ext", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:allowed-ext/net-1", 1).await;
    seed(&node, "acme", "ext:forbidden-ext/net-1", 2).await;

    let p = admin("acme", &["mcp:allowed-ext.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();

    assert_eq!(row(&out, "ext:allowed-ext/net-1")["state"], "reported");
    assert_eq!(row(&out, "ext:forbidden-ext/net-1")["state"], "denied");
    assert_eq!(
        row(&out, "ext:forbidden-ext/net-1")["missing_cap"],
        "mcp:forbidden-ext.ingest.health:call"
    );
}

// ------------------------------------------------------------ mandatory: workspace isolation ----

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_producer_in_another_workspace_is_never_reported() {
    // The registry is node-wide, so the extension is reachable from both workspaces — which is
    // exactly why this test matters: the wall has to come from the SAMPLES, not from discovery.
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "acme", "ext:demo-probe/acme-net", 1).await;
    seed(&node, "other", "ext:demo-probe/other-net", 1).await;

    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let out = health(&node, &p, "acme").await.unwrap();
    let producers: Vec<&str> = out["producers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["producer"].as_str().unwrap())
        .collect();

    assert_eq!(producers, vec!["ext:demo-probe/acme-net"]);
    assert!(
        !producers.iter().any(|p| p.contains("other-net")),
        "ws `other`'s producer leaked into ws `acme`: {producers:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_principal_cannot_read_producer_health_across_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    register_reporter(&node, "demo-probe", r#"{"state":"connected"}"#, false);
    seed(&node, "other", "ext:demo-probe/other-net", 1).await;

    // A ws-`acme` principal asking about ws `other` — the wall is checked before the cap.
    let p = admin("acme", &["mcp:demo-probe.ingest.health:call"]);
    let err = health(&node, &p, "other").await.unwrap_err();
    assert!(matches!(err, ToolError::Denied), "got {err:?}");
}
