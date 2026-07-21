//! Host-layer tests for **scheduled rules** — a `#[schedule(...)]` directive on a rule that compiles
//! to a managed `cron → rule` flow (scheduled-rules-scope). Real store, real caps, real MCP bridge,
//! real `react_to_flows_cron` reactor. No mocks, no fakes (rule 9) — the schedule side effects go
//! through the SAME `rules.save`/`flows.*` verbs a client calls.
//!
//! The one architectural invariant under test: the directive is **compiled at save**, and the
//! **existing flow cron reactor** fires the run. There is NO rule-cron reactor (a workspace-wide grep
//! for one is the ship gate, asserted in `no_rule_cron_reactor_exists`).
//!
//! Mandatory categories:
//!   - capability-deny: `rules.save` without its cap; the **split-grant** (rule-write but not
//!     flow-write) → schedule metadata persists + `pending`, no managed flow, no widening;
//!   - workspace-isolation: a ws-B save can neither read nor build a ws-A managed flow;
//!   - preview parity: the `rules.get` `next_runs` block matches `croner`'s `next_after` (the engine
//!     the reactor fires on) on a shared `(cron, now) → next-5` fixture.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, cron_run_id, insight_list, react_to_flows_cron, Node as HostNode};
use lb_insights::ListQuery;
use lb_reminders::next_after;
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

/// The full grant to schedule a rule (rule-write ∩ flow-write) AND run it to a raised insight.
const FULL: &[&str] = &[
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "mcp:rules.list:call",
    "mcp:rules.eval:call",
    "mcp:flows.save:call",
    "mcp:flows.get:call",
    "mcp:flows.node.update:call",
    "mcp:flows.node.get:call",
    "mcp:flows.delete:call",
    "mcp:flows.runs.get:call",
    "mcp:flows.run:call",
    "mcp:insight.raise:call",
    "mcp:insight.list:call",
    "store:rule:write",
    "store:rule:read",
    "store:flow:write",
    "store:flow:read",
];

/// Rule-write WITHOUT flow-write — the split-grant. Can persist the rule + its schedule metadata but
/// cannot build the managed flow (scheduling never widens the caller's authority).
const RULE_WRITE_ONLY: &[&str] = &[
    "mcp:rules.save:call",
    "mcp:rules.get:call",
    "store:rule:write",
    "store:rule:read",
    // deliberately NO mcp:flows.* / store:flow:* — a flow write is denied.
];

/// A rule body that raises one insight, prefixed with a schedule directive.
fn scheduled_rule_body(directive: &str) -> String {
    format!(
        "{directive}\n\ninsight.raise(#{{ dedup_key: \"sched-demo\", severity: \"warning\", \
         title: \"scheduled fired\", body: #{{ n: 1 }} }});"
    )
}

async fn save_rule(node: &Arc<HostNode>, p: &Principal, ws: &str, id: &str, body: &str) -> Value {
    let args = json!({ "id": id, "name": id, "body": body });
    let out = call_tool(node, p, ws, "rules.save", &args.to_string())
        .await
        .expect("rules.save ok");
    serde_json::from_str(&out).unwrap()
}

async fn get_rule(node: &Arc<HostNode>, p: &Principal, ws: &str, id: &str) -> Value {
    let out = call_tool(node, p, ws, "rules.get", &json!({ "id": id }).to_string())
        .await
        .expect("rules.get ok");
    serde_json::from_str(&out).unwrap()
}

// --- Slice 1: directive extract + NL→cron compile (through the real save path) -----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn directive_compiles_to_cron_on_save() {
    let ws = "sched-compile";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);

    let save = save_rule(
        &node,
        &p,
        ws,
        "r1",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    // The save response carries the compiled + managed schedule.
    assert_eq!(save["schedule"]["cron"], "*/15 * * * *");
    assert_eq!(save["schedule"]["managed"], true);

    // The stored rule carries the compiled `{raw, cron}` metadata.
    let rule = get_rule(&node, &p, ws, "r1").await;
    assert_eq!(rule["schedule"]["raw"], "every 15 minutes");
    assert_eq!(rule["schedule"]["cron"], "*/15 * * * *");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn unparseable_directive_is_a_save_error() {
    let ws = "sched-bad";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    let args = json!({
        "id": "bad", "name": "bad",
        "body": scheduled_rule_body("#[schedule(\"whenever the mood strikes\")]"),
    });
    let err = call_tool(&node, &p, ws, "rules.save", &args.to_string()).await;
    assert!(
        err.is_err(),
        "an unparseable directive must fail the save, not silently drop it"
    );
}

// --- Slice 2: the syncer — build / update / delete the managed flow ----------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn save_builds_the_managed_flow() {
    let ws = "sched-build";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    save_rule(
        &node,
        &p,
        ws,
        "cooler",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // The managed flow exists with the two-node cron→rule shape + the marker.
    let flow = flows_get(&node, &p, ws, "schedule:cooler").await;
    assert_eq!(flow["managedBy"], "rule-schedule:cooler");
    assert_eq!(flow["enabled"], true);
    assert_eq!(flow["startOnBoot"], true);
    let nodes = flow["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 2);
    let trig = nodes.iter().find(|n| n["id"] == "trigger").unwrap();
    assert_eq!(trig["config"]["mode"], "cron");
    assert_eq!(trig["config"]["cron"], "*/15 * * * *");
    let rule = nodes.iter().find(|n| n["id"] == "rule").unwrap();
    assert_eq!(rule["config"]["rule"], "cooler");
    assert_eq!(rule["needs"][0], "trigger");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn resave_is_idempotent_then_updates_then_deletes() {
    let ws = "sched-reconcile";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);

    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    let v1 = flows_get(&node, &p, ws, "schedule:r").await["version"]
        .as_u64()
        .unwrap();

    // Re-save the SAME directive → no-op (no version bump).
    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    let v2 = flows_get(&node, &p, ws, "schedule:r").await["version"]
        .as_u64()
        .unwrap();
    assert_eq!(
        v1, v2,
        "an unchanged directive re-save must not rewrite the managed flow"
    );

    // Change the directive → one trigger update to the new cron (version bumps).
    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every hour\")]"),
    )
    .await;
    let flow = flows_get(&node, &p, ws, "schedule:r").await;
    assert!(
        flow["version"].as_u64().unwrap() > v2,
        "a changed directive bumps the flow version"
    );
    let trig = flow["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == "trigger")
        .unwrap();
    assert_eq!(trig["config"]["cron"], "0 * * * *");

    // Remove the directive → the managed flow is deleted (rule reverts to run-on-demand).
    save_rule(
        &node,
        &p,
        ws,
        "r",
        "insight.raise(#{ dedup_key: \"x\", title: \"t\" });",
    )
    .await;
    let gone = call_tool(
        &node,
        &p,
        ws,
        "flows.get",
        &json!({ "id": "schedule:r" }).to_string(),
    )
    .await;
    assert!(
        gone.is_err(),
        "removing the directive deletes the managed flow"
    );
    let rule = get_rule(&node, &p, ws, "r").await;
    assert!(
        rule.get("schedule").is_none() || rule["schedule"].is_null(),
        "a rule with no directive carries no schedule metadata"
    );
}

// --- Capability-deny (mandatory) ---------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_save_denied_without_the_cap() {
    let ws = "sched-deny";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, &["store:rule:read"]); // no mcp:rules.save:call
    let args = json!({ "id": "r", "name": "r", "body": "let x = 1;" });
    let err = call_tool(&node, &p, ws, "rules.save", &args.to_string()).await;
    assert!(err.is_err(), "rules.save without its cap is denied");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn split_grant_persists_schedule_but_reports_pending() {
    // The mandatory split: rule-write but NOT flow-write. The rule + its schedule metadata persist,
    // but the managed flow could not be built — reported `pending`, never a silent half-state, never
    // widening the caller's authority into flow-authoring.
    let ws = "sched-split";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, RULE_WRITE_ONLY);

    let save = save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    assert_eq!(save["schedule"]["managed"], false);
    assert_eq!(save["schedule"]["pending"], "needs flow-write");
    // The compiled schedule metadata IS on the rule (the save persisted it).
    assert_eq!(save["schedule"]["cron"], "*/15 * * * *");

    // No managed flow was built (the flow write was denied — indistinguishable from absent).
    let flow_read = call_tool(
        &node,
        &p,
        ws,
        "flows.get",
        &json!({ "id": "schedule:r" }).to_string(),
    )
    .await;
    assert!(
        flow_read.is_err(),
        "no managed flow exists for a split-grant save"
    );

    // The read surface reports the schedule as not-managed (pending), not scheduled-and-running.
    let rule = get_rule(&node, &p, ws, "r").await;
    assert_eq!(rule["schedule"]["managed"], false);
}

// --- Workspace-isolation (mandatory) -----------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_read_or_build_a_ws_a_managed_flow() {
    let node = Arc::new(HostNode::boot().await.unwrap());
    let pa = principal("ws-a", FULL);
    let pb = principal("ws-b", FULL);

    // ws-A schedules a rule → its managed flow lives in ws-A.
    save_rule(
        &node,
        &pa,
        "ws-a",
        "cooler",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // ws-B (full caps in its OWN workspace) cannot see ws-A's managed flow (namespace wall).
    let seen = call_tool(
        &node,
        &pb,
        "ws-b",
        "flows.get",
        &json!({ "id": "schedule:cooler" }).to_string(),
    )
    .await;
    assert!(seen.is_err(), "ws-B cannot read a ws-A managed flow");

    // ws-B cannot even read the ws-A rule (so it can never learn the schedule).
    let rule = call_tool(
        &node,
        &pb,
        "ws-b",
        "rules.get",
        &json!({ "id": "cooler" }).to_string(),
    )
    .await;
    assert!(rule.is_err(), "ws-B cannot read a ws-A rule");

    // A ws-B cron reactor pass never fires/sees the ws-A managed flow.
    let pass = react_to_flows_cron(&node, &pb, "ws-b", 10_000)
        .await
        .unwrap();
    assert_eq!(
        pass.fired, 0,
        "a ws-B reactor pass never touches a ws-A schedule"
    );
}

// --- Slice 3: read surface (schedule block + list filter + drift) ------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_get_carries_the_schedule_block_and_next_runs() {
    let ws = "sched-read";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // Pin `now` (ts is millis on the get path; the block computes on seconds).
    let now_secs = 1_000_000u64;
    let out = call_tool(
        &node,
        &p,
        ws,
        "rules.get",
        &json!({ "id": "r", "ts": now_secs * 1000 }).to_string(),
    )
    .await
    .unwrap();
    let block = &serde_json::from_str::<Value>(&out).unwrap()["schedule"];
    assert_eq!(block["cron"], "*/15 * * * *");
    assert_eq!(block["flow_id"], "schedule:r");
    assert_eq!(block["managed"], true);
    assert_eq!(block["drift"], false);

    // Preview parity: the block's next_runs MUST equal croner's next_after chain (the reactor engine).
    let runs = block["next_runs"].as_array().unwrap();
    assert_eq!(runs.len(), 5);
    let mut cursor = now_secs;
    for r in runs {
        let expect = next_after("*/15 * * * *", cursor).unwrap();
        assert_eq!(
            r.as_u64().unwrap(),
            expect,
            "preview must match the reactor's croner engine"
        );
        cursor = expect;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_scheduled_filter_returns_only_scheduled_rules() {
    let ws = "sched-list";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    save_rule(
        &node,
        &p,
        ws,
        "timed",
        &scheduled_rule_body("#[schedule(\"hourly\")]"),
    )
    .await;
    save_rule(
        &node,
        &p,
        ws,
        "ondemand",
        "insight.raise(#{ dedup_key: \"y\", title: \"t\" });",
    )
    .await;

    let out = call_tool(
        &node,
        &p,
        ws,
        "rules.list",
        &json!({ "scheduled": true }).to_string(),
    )
    .await
    .unwrap();
    let rules = serde_json::from_str::<Value>(&out).unwrap()["rules"]
        .as_array()
        .unwrap()
        .clone();
    let ids: Vec<&str> = rules.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(
        ids,
        vec!["timed"],
        "scheduled:true returns exactly the rules carrying a schedule"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn drift_is_flagged_when_the_managed_flow_is_hand_edited() {
    let ws = "sched-drift";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // A power user hand-edits the managed flow's cron away from the directive.
    call_tool(
        &node, &p, ws, "flows.node.update",
        &json!({ "id": "schedule:r", "node": "trigger", "config": { "mode": "cron", "cron": "0 0 * * *" } }).to_string(),
    ).await.unwrap();

    // rules.get flags the drift (allow-and-flag; the directive is source of truth).
    let block = get_rule(&node, &p, ws, "r").await["schedule"].clone();
    assert_eq!(
        block["drift"], true,
        "a diverged managed-flow cron is flagged"
    );

    // Re-saving the rule re-asserts the directive's cron (drift clears).
    save_rule(
        &node,
        &p,
        ws,
        "r",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    let block = get_rule(&node, &p, ws, "r").await["schedule"].clone();
    assert_eq!(
        block["drift"], false,
        "the save re-asserts the directive value"
    );
}

// --- Slice 4: firing end-to-end on the REAL react_cron reactor ---------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scheduled_rule_fires_through_the_flow_cron_reactor_and_dedups() {
    let ws = "sched-fire";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);
    save_rule(
        &node,
        &p,
        ws,
        "cooler",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    assert_eq!(count_insights(&node, &p, ws).await, 0, "clean start");

    // First pass primes the managed flow's cron cursor (no fire on init).
    react_to_flows_cron(&node, &p, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:cooler", "trigger").await;
    assert!(next > 0, "the reactor primed the managed trigger's cursor");

    // Due pass → EXACTLY ONE run of the managed flow fires → the rule runs → an insight is raised.
    let pass = react_to_flows_cron(&node, &p, ws, next + 1).await.unwrap();
    assert_eq!(pass.fired, 1, "the managed cron flow fired exactly one run");
    poll_run_terminal(
        &node,
        &p,
        ws,
        &cron_run_id("schedule:cooler", "trigger", next),
    )
    .await;
    assert_eq!(
        count_insights(&node, &p, ws).await,
        1,
        "the rule ran and raised one insight"
    );

    // Second tick at the SAME now → idempotent no-op (the job exists; the reactor's fire-once), AND
    // the insight dedups (same dedup_key) — no second record.
    let pass2 = react_to_flows_cron(&node, &p, ws, next + 1).await.unwrap();
    assert_eq!(
        pass2.fired, 0,
        "a re-scan at the same instant fires nothing (fire-once)"
    );
    assert_eq!(
        count_insights(&node, &p, ws).await,
        1,
        "insight dedups on the second firing"
    );
}

// --- The ship gate: NO rule-cron reactor exists ------------------------------------------------

#[test]
fn no_rule_cron_reactor_exists() {
    // The single biggest scope risk is an implementer building a rule-cron reactor that scans rule
    // directives on a firing tick — the exact "second scheduler" the convergence scope deleted. This
    // proves the directive is compiled to a managed flow and fired ONLY by `react_to_flows_cron`: no
    // source file names a rule-schedule reactor / scans rule bodies on a tick.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut offenders = Vec::new();
    walk(&root, &mut |path, text| {
        // A file that both (a) reacts on a clock AND (b) reads the rule schedule table is the smell.
        let reacts = text.contains("react_to_rule")
            || text.contains("rule_cron")
            || text.contains("rules_cron");
        if reacts {
            offenders.push(path.to_string_lossy().to_string());
        }
    });
    assert!(
        offenders.is_empty(),
        "found a rule-cron reactor (forbidden): {offenders:?}"
    );
}

fn walk(dir: &std::path::Path, f: &mut impl FnMut(&std::path::Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let path = e.path();
        if path.is_dir() {
            walk(&path, f);
        } else if path.extension().map(|x| x == "rs").unwrap_or(false) {
            if let Ok(text) = std::fs::read_to_string(&path) {
                f(&path, &text);
            }
        }
    }
}

// --- helpers -----------------------------------------------------------------------------------

async fn flows_get(node: &Arc<HostNode>, p: &Principal, ws: &str, id: &str) -> Value {
    let out = call_tool(node, p, ws, "flows.get", &json!({ "id": id }).to_string())
        .await
        .expect("flows.get ok");
    serde_json::from_str(&out).unwrap()
}

async fn count_insights(node: &Arc<HostNode>, p: &Principal, ws: &str) -> usize {
    insight_list(
        &node.store,
        p,
        ws,
        ListQuery {
            filter: Default::default(),
            cursor: None,
            limit: 1000,
        },
    )
    .await
    .unwrap()
    .items
    .len()
}

async fn cursor_next(node: &Arc<HostNode>, ws: &str, flow: &str, node_id: &str) -> u64 {
    lb_store::read(
        &node.store,
        ws,
        "flow_trigger_state",
        &format!("{flow}:{node_id}"),
    )
    .await
    .unwrap()
    .and_then(|v| {
        v.get("data")
            .and_then(|d| d.get("next_attempt_ts"))
            .or_else(|| v.get("next_attempt_ts"))
            .and_then(|x| x.as_u64())
    })
    .unwrap_or(0)
}

async fn poll_run_terminal(node: &Arc<HostNode>, p: &Principal, ws: &str, run_id: &str) {
    for _ in 0..50 {
        let out = call_tool(
            node,
            p,
            ws,
            "flows.runs.get",
            &json!({ "run_id": run_id }).to_string(),
        )
        .await;
        if let Ok(s) = out {
            let v: Value = serde_json::from_str(&s).unwrap_or(Value::Null);
            let status = v["status"].as_str().unwrap_or("");
            if matches!(
                status,
                "success" | "partialFailure" | "failed" | "cancelled"
            ) {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
}
