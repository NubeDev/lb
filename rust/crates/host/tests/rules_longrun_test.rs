//! Host-layer tests for job-backed rule runs (long-running-rules-scope Testing plan). Real store,
//! real caps, real MCP bridge, real `lb-jobs` records; the run body is evaluated by the real engine
//! on a real worker task. Mandatory categories: capability-deny (each `rules.runs.*` verb + read ≠
//! control), workspace-isolation (ws-B cannot see/control a ws-A run), pause/resume (the headline:
//! suspend mid-run, resume replays checkpoints without re-spends), restart-resume (a suspended run
//! resumes on a fresh Node over the same store), and cooperative cancel.

use std::sync::Arc;
use std::time::Duration;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// The full long-run grant: start + observe + control, plus the channel caps the exactly-once
/// replay test posts with.
const FULL_RUNS: &[&str] = &[
    "mcp:rules.run_async:call",
    "mcp:rules.runs.get:call",
    "mcp:rules.runs.list:call",
    "mcp:rules.runs.suspend:call",
    "mcp:rules.runs.resume:call",
    "mcp:rules.runs.cancel:call",
    "mcp:channel.post:call",
    "mcp:channel.history:call",
    "bus:chan/*:pub",
    "bus:chan/*:sub",
];

async fn call(node: &Arc<Node>, p: &Principal, ws: &str, tool: &str, input: Value) -> Value {
    let out = call_tool(node, p, ws, tool, &input.to_string())
        .await
        .unwrap_or_else(|e| panic!("{tool} failed: {e:?}"));
    serde_json::from_str(&out).unwrap()
}

/// Poll `rules.runs.get` until `pred` holds (or panic after ~10 s — a hung run is a test failure,
/// never a silent pass).
async fn wait_for(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    run_id: &str,
    what: &str,
    pred: impl Fn(&Value) -> bool,
) -> Value {
    for _ in 0..200 {
        let got = call(node, p, ws, "rules.runs.get", json!({ "run_id": run_id })).await;
        if pred(&got) {
            return got;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for {what} on {run_id}");
}

/// A body that spins cooperatively until suspended/cancelled — the pausable long run.
const SPIN: &str = r#"while !job.should_stop() { }"#;

// ----- capability-deny ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn each_runs_verb_is_denied_without_its_cap() {
    let ws = "lr-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Holds an unrelated rules cap — every runs verb still denies (one cap per verb).
    let p = principal(ws, &["mcp:rules.help:call"]);
    for (tool, input) in [
        ("rules.run_async", json!({ "body": "1" })),
        ("rules.runs.get", json!({ "run_id": "x" })),
        ("rules.runs.list", json!({})),
        ("rules.runs.suspend", json!({ "run_id": "x" })),
        ("rules.runs.resume", json!({ "run_id": "x" })),
        ("rules.runs.cancel", json!({ "run_id": "x" })),
    ] {
        let err = call_tool(&node, &p, ws, tool, &input.to_string()).await;
        assert!(err.is_err(), "{tool} must deny without its cap");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn read_caps_do_not_grant_control() {
    // Observer role: get/list only. Start a run with a full principal, then prove the observer can
    // watch it but not suspend/resume/cancel it (read ≠ control, job-control doctrine).
    let ws = "lr-observer";
    let node = Arc::new(Node::boot().await.unwrap());
    let owner = principal(ws, FULL_RUNS);
    let observer = principal(ws, &["mcp:rules.runs.get:call", "mcp:rules.runs.list:call"]);

    let started = call(
        &node,
        &owner,
        ws,
        "rules.run_async",
        json!({ "body": SPIN, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    wait_for(&node, &observer, ws, &run_id, "live", |v| {
        v["live"].as_bool() == Some(true)
    })
    .await;

    for tool in [
        "rules.runs.suspend",
        "rules.runs.resume",
        "rules.runs.cancel",
    ] {
        let err = call_tool(
            &node,
            &observer,
            ws,
            tool,
            &json!({ "run_id": run_id }).to_string(),
        )
        .await;
        assert!(err.is_err(), "{tool} must deny for a read-only principal");
    }
    // Clean up the spinner.
    call(
        &node,
        &owner,
        ws,
        "rules.runs.cancel",
        json!({ "run_id": run_id }),
    )
    .await;
}

// ----- workspace-isolation -------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ws_b_cannot_see_or_control_a_ws_a_run() {
    let node = Arc::new(Node::boot().await.unwrap());
    let pa = principal("lr-ws-a", FULL_RUNS);
    let pb = principal("lr-ws-b", FULL_RUNS);

    let started = call(
        &node,
        &pa,
        "lr-ws-a",
        "rules.run_async",
        json!({ "body": "1 + 1", "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    wait_for(&node, &pa, "lr-ws-a", &run_id, "done", |v| {
        v["status"] == "done"
    })
    .await;

    // ws-B: the run does not exist — get/suspend/resume/cancel all opaque-fail, list is empty.
    for tool in [
        "rules.runs.get",
        "rules.runs.suspend",
        "rules.runs.resume",
        "rules.runs.cancel",
    ] {
        let err = call_tool(
            &node,
            &pb,
            "lr-ws-b",
            tool,
            &json!({ "run_id": run_id }).to_string(),
        )
        .await;
        assert!(err.is_err(), "{tool} must not cross the workspace wall");
    }
    let listed = call(&node, &pb, "lr-ws-b", "rules.runs.list", json!({})).await;
    assert_eq!(listed["items"].as_array().unwrap().len(), 0);
}

// ----- the headline: suspend mid-run, resume replays checkpoints without re-spends ---------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspend_mid_run_then_resume_finishes_without_respending_steps() {
    let ws = "lr-pause-resume";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL_RUNS);

    // Attempt 1: memoizes `a` (with a log marker), posts an id-pinned message, opens the gate,
    // then spins. Resume: the gate checkpoint short-circuits the spin, `a` replays as a lookup
    // (no marker in the final attempt's log), `b` runs, the re-posted message upserts (same id).
    let body = r#"
        let a = job.step("a", || { log("ran-a"); 1 });
        channel.post("ops", #{ id: "lr-once", body: "exactly once" });
        if !job.has("gate") {
            job.set("gate", true);
            while !job.should_stop() { }
        }
        a + job.step("b", || 2)
    "#;
    let started = call(
        &node,
        &p,
        ws,
        "rules.run_async",
        json!({ "body": body, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();

    // It reaches the spin (the gate checkpoint is durable), then parks on suspend.
    wait_for(&node, &p, ws, &run_id, "gate checkpoint", |v| {
        v["checkpoints"]
            .as_array()
            .is_some_and(|c| c.iter().any(|k| k == "gate"))
    })
    .await;
    call(
        &node,
        &p,
        ws,
        "rules.runs.suspend",
        json!({ "run_id": run_id }),
    )
    .await;
    let parked = wait_for(&node, &p, ws, &run_id, "suspended", |v| {
        v["status"] == "suspended"
    })
    .await;
    assert_eq!(parked["live"], false, "the worker exited on park");
    assert!(
        parked["checkpoints"]
            .as_array()
            .unwrap()
            .iter()
            .any(|k| k == "a"),
        "the memoized step survived the pause"
    );

    // Resume: replays over the checkpoints and completes.
    call(
        &node,
        &p,
        ws,
        "rules.runs.resume",
        json!({ "run_id": run_id }),
    )
    .await;
    let done = wait_for(&node, &p, ws, &run_id, "done", |v| v["status"] == "done").await;

    // The result: 1 + 2, computed with `a` as a LOOKUP — its log marker must be absent from the
    // finishing attempt (no re-spend).
    assert_eq!(done["result"]["output"]["value"], json!(3));
    let log = done["result"]["log"].as_array().unwrap();
    assert!(
        !log.iter().any(|l| l["message"] == "ran-a"),
        "step `a` must not re-run on resume; log was {log:?}"
    );

    // Exactly-once effect: the replayed post landed on the SAME deterministic id — one message.
    let hist = call(&node, &p, ws, "channel.history", json!({ "cid": "ops" })).await;
    let msgs = hist["messages"].as_array().unwrap();
    assert_eq!(msgs.len(), 1, "replayed post must upsert, not duplicate");
    assert_eq!(msgs[0]["id"], "lr-once");
}

// ----- cooperative cancel --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_bites_mid_run_and_is_idempotent() {
    let ws = "lr-cancel";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL_RUNS);

    let started = call(
        &node,
        &p,
        ws,
        "rules.run_async",
        json!({ "body": SPIN, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    wait_for(&node, &p, ws, &run_id, "live", |v| {
        v["live"].as_bool() == Some(true)
    })
    .await;

    call(
        &node,
        &p,
        ws,
        "rules.runs.cancel",
        json!({ "run_id": run_id }),
    )
    .await;
    wait_for(&node, &p, ws, &run_id, "cancelled", |v| {
        v["status"] == "cancelled"
    })
    .await;

    // Idempotent re-cancel (D2): a clean no-op reporting the honest status.
    let again = call(
        &node,
        &p,
        ws,
        "rules.runs.cancel",
        json!({ "run_id": run_id }),
    )
    .await;
    assert_eq!(again["status"], "cancelled");

    // A cancelled run is not resumable — clean author error, not a silent restart.
    let err = call_tool(
        &node,
        &p,
        ws,
        "rules.runs.resume",
        &json!({ "run_id": run_id }).to_string(),
    )
    .await;
    assert!(err.is_err(), "resume of a cancelled run must refuse");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_of_a_suspended_run_works() {
    // Job-control D2: cancel is terminal from any non-final state, including suspended.
    let ws = "lr-cancel-suspended";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL_RUNS);

    let started = call(
        &node,
        &p,
        ws,
        "rules.run_async",
        json!({ "body": SPIN, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    wait_for(&node, &p, ws, &run_id, "live", |v| {
        v["live"].as_bool() == Some(true)
    })
    .await;
    call(
        &node,
        &p,
        ws,
        "rules.runs.suspend",
        json!({ "run_id": run_id }),
    )
    .await;
    wait_for(&node, &p, ws, &run_id, "suspended", |v| {
        v["status"] == "suspended"
    })
    .await;

    let out = call(
        &node,
        &p,
        ws,
        "rules.runs.cancel",
        json!({ "run_id": run_id }),
    )
    .await;
    assert_eq!(out["status"], "cancelled");
}

// ----- run/list/progress shapes ---------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn progress_and_result_surface_in_get_and_list() {
    let ws = "lr-observe";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL_RUNS);

    let body = r#"
        job.progress(10, "planning");
        job.set("plan", ["x"]);
        job.progress(90, "almost");
        "report"
    "#;
    let started = call(
        &node,
        &p,
        ws,
        "rules.run_async",
        json!({ "body": body, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    let done = wait_for(&node, &p, ws, &run_id, "done", |v| v["status"] == "done").await;

    assert_eq!(done["progress"]["pct"], 90, "latest beat wins");
    assert_eq!(done["progress"]["msg"], "almost");
    assert_eq!(done["result"]["output"]["value"], json!("report"));
    assert!(done["checkpoints"]
        .as_array()
        .unwrap()
        .iter()
        .any(|k| k == "plan"));
    assert!(done["tail"].as_array().unwrap().len() >= 3);

    let listed = call(&node, &p, ws, "rules.runs.list", json!({})).await;
    let items = listed["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["run_id"], run_id.as_str());
    assert_eq!(items[0]["status"], "done");
    // The list row is the light shape — no result/tail payload.
    assert!(items[0].get("result").is_none());

    let filtered = call(
        &node,
        &p,
        ws,
        "rules.runs.list",
        json!({ "status": "cancelled" }),
    )
    .await;
    assert_eq!(filtered["items"].as_array().unwrap().len(), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_failing_body_settles_failed_with_the_error_recorded() {
    let ws = "lr-fail";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, FULL_RUNS);

    let started = call(
        &node,
        &p,
        ws,
        "rules.run_async",
        json!({ "body": r#"throw "boom""#, "ts": 7 }),
    )
    .await;
    let run_id = started["run_id"].as_str().unwrap().to_string();
    let failed = wait_for(&node, &p, ws, &run_id, "failed", |v| {
        v["status"] == "failed"
    })
    .await;
    assert!(
        failed["error"].as_str().unwrap().contains("boom"),
        "the author error is recorded on the run"
    );
}

// ----- offline/sync: a suspended run resumes on a fresh Node over the same store ------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspended_run_resumes_after_a_restart() {
    let ws = "lr-restart";
    let dir = std::env::temp_dir().join(format!("lb-rules-longrun-restart-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let body = r#"
        let a = job.step("a", || 40);
        if !job.has("gate") {
            job.set("gate", true);
            while !job.should_stop() { }
        }
        a + job.step("b", || 2)
    "#;
    let run_id;
    {
        let store = lb_store::Store::open(&path).await.unwrap();
        let node = Arc::new(Node::boot_with_store(store).await.unwrap());
        let p = principal(ws, FULL_RUNS);
        let started = call(
            &node,
            &p,
            ws,
            "rules.run_async",
            json!({ "body": body, "ts": 7 }),
        )
        .await;
        run_id = started["run_id"].as_str().unwrap().to_string();
        wait_for(&node, &p, ws, &run_id, "gate checkpoint", |v| {
            v["checkpoints"]
                .as_array()
                .is_some_and(|c| c.iter().any(|k| k == "gate"))
        })
        .await;
        call(
            &node,
            &p,
            ws,
            "rules.runs.suspend",
            json!({ "run_id": run_id }),
        )
        .await;
        wait_for(&node, &p, ws, &run_id, "suspended", |v| {
            v["status"] == "suspended"
        })
        .await;
        // Node dropped here — the "restart".
    }
    {
        let store = lb_store::Store::open(&path).await.unwrap();
        let node = Arc::new(Node::boot_with_store(store).await.unwrap());
        let p = principal(ws, FULL_RUNS);
        // The fresh node sees the parked run (live:false) with its checkpoints intact.
        let parked = call(&node, &p, ws, "rules.runs.get", json!({ "run_id": run_id })).await;
        assert_eq!(parked["status"], "suspended");
        assert_eq!(parked["live"], false);

        call(
            &node,
            &p,
            ws,
            "rules.runs.resume",
            json!({ "run_id": run_id }),
        )
        .await;
        let done = wait_for(&node, &p, ws, &run_id, "done", |v| v["status"] == "done").await;
        assert_eq!(done["result"]["output"]["value"], json!(42));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
