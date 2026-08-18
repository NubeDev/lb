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
use lb_host::{
    call_tool, cron_run_id, insight_list, owner_principal, react_to_flows_cron, reactor_caps,
    Node as HostNode,
};

/// The cap a `query("<datasource>", ..)` rule body needs at collect, deliberately absent from
/// `reactor_caps()` (blanket datasource reach for every scheduled flow on the node).
const FEDERATION_QUERY: &str = "mcp:federation.query:call";
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
    "mcp:inbox.list:call",
    "mcp:rules.delete:call",
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

/// **The reactor's OWN authority fires the rule** (lb#167 regression).
///
/// The test above drives `react_to_flows_cron` with `p` — the author's FULL principal, which holds
/// `mcp:rules.eval:call`. Production does not: `spawn_flow_reactors` mints
/// `Principal::routed("node:reactor", ws, reactor_caps())` on every tick. That substitution is why a
/// green suite coexisted with six consecutive `partialFailure` fires on a live node, every one with
/// the rule step `"error":"denied"`.
///
/// So this fires the SAME managed flow under the reactor's real caps. It fails before the
/// `reactor_caps()` fix (insight count stays 0, run is `partialFailure`) and passes after.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scheduled_rule_fires_under_the_reactors_own_principal_not_the_authors() {
    let ws = "sched-reactor-caps";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    // Authoring still happens as the user (a save needs rule-write ∩ flow-write).
    save_rule(
        &node,
        &author,
        ws,
        "cooler",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    assert_eq!(count_insights(&node, &author, ws).await, 0, "clean start");

    // …but FIRING happens as the node, under the REAL production principal — minted exactly as
    // `spawn_flow_reactors` does, from `reactor_caps()` itself. Importing the function rather than
    // copying its list is the whole point: a mirrored list would keep passing after someone removes
    // a grant, which is the failure mode that let lb#167 ship green.
    let reactor = Principal::routed("node:reactor", ws, reactor_caps());

    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:cooler", "trigger").await;
    assert!(next > 0, "the reactor primed the managed trigger's cursor");

    let pass = react_to_flows_cron(&node, &reactor, ws, next + 1)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "the managed cron flow fired exactly one run");

    let run_id = cron_run_id("schedule:cooler", "trigger", next);
    poll_run_terminal(&node, &author, ws, &run_id).await;

    // The step-level assertion is the point: `partialFailure` with `rule → denied` is precisely the
    // shape lb#167 produced, and a bare insight-count check would not say WHY it was zero.
    let run = call_tool(
        &node,
        &author,
        ws,
        "flows.runs.get",
        &json!({ "run_id": run_id }).to_string(),
    )
    .await
    .expect("flows.runs.get ok");
    let run: Value = serde_json::from_str(&run).unwrap();
    let rule_step = run["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|s| s["id"] == "rule")
        .expect("the managed flow has a rule step")
        .clone();
    assert_ne!(
        rule_step["error"],
        json!("denied"),
        "the rule node was DENIED under the reactor's principal — this is lb#167: \
         `reactor_caps()` is missing `mcp:rules.eval:call`"
    );
    assert_eq!(rule_step["outcome"], json!("ok"), "the rule step ran");
    assert_eq!(run["status"], json!("success"), "no partialFailure");

    // …and the effect landed. The insight is the only durable proof the cage actually executed.
    assert_eq!(
        count_insights(&node, &author, ws).await,
        1,
        "the scheduled rule raised its insight under the reactor's own authority"
    );
}

/// **An ALERTING scheduled rule, under the reactor's own authority** (lb#167, second half).
///
/// `insight.raise(...)` is one of a rule's two finishing moves; `alert(...)` is the other, and it
/// takes a DIFFERENT door: the host fans every alert-marked finding out to the inbox + outbox at the
/// end of a successful eval (`rules::run::route_alerts`), under the calling principal. A deny there
/// fails the whole `rules.eval`, so granting `mcp:rules.eval:call` alone leaves an alerting rule
/// `denied` on every fire — the same asymmetry, one verb deeper.
///
/// Same construction as the test above and for the same reason: the principal comes from
/// `reactor_caps()` itself, so removing either grant fails this test rather than silently un-shipping
/// the fix. Fails without `mcp:inbox.record:call` / `mcp:outbox.enqueue:call`; passes with them.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scheduled_alerting_rule_routes_to_inbox_under_the_reactors_own_principal() {
    let ws = "sched-reactor-alert";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    let body = "#[schedule(\"every 15 minutes\")]\n\n\
                alert(#{ level: \"warning\", title: \"scheduled alert\", n: 1 });";
    save_rule(&node, &author, ws, "alerter", body).await;
    assert_eq!(
        count_inbox(&node, &author, ws).await,
        0,
        "clean start: nothing on the rules channel"
    );

    let reactor = Principal::routed("node:reactor", ws, reactor_caps());
    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:alerter", "trigger").await;
    let pass = react_to_flows_cron(&node, &reactor, ws, next + 1)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "the managed cron flow fired exactly one run");

    let run_id = cron_run_id("schedule:alerter", "trigger", next);
    poll_run_terminal(&node, &author, ws, &run_id).await;

    let run = call_tool(
        &node,
        &author,
        ws,
        "flows.runs.get",
        &json!({ "run_id": run_id }).to_string(),
    )
    .await
    .expect("flows.runs.get ok");
    let run: Value = serde_json::from_str(&run).unwrap();
    let rule_step = run["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|s| s["id"] == "rule")
        .expect("the managed flow has a rule step")
        .clone();
    assert_ne!(
        rule_step["error"],
        json!("denied"),
        "the alerting rule was DENIED under the reactor's principal — `reactor_caps()` is missing \
         `mcp:inbox.record:call` / `mcp:outbox.enqueue:call` (lb#167)"
    );
    assert_eq!(run["status"], json!("success"), "no partialFailure");

    // The routed item is the durable proof the fan-out ran, not just that the cage did.
    assert_eq!(
        count_inbox(&node, &author, ws).await,
        1,
        "the scheduled alert landed on the inbox's `rules` channel"
    );
}

// --- Deleting a scheduled rule tears its cron down (lb#167 follow-up) ---------------------------

/// A deleted rule must not leave its managed cron behind. Before the fix `rules.delete` tombstoned
/// only the rule record, so `schedule:{id}` kept firing on its own schedule at a rule that no longer
/// existed — an orphan with no owner left to delete it through the rules surface.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn rules_delete_tears_down_the_managed_flow() {
    let ws = "sched-delete";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let p = principal(ws, FULL);

    save_rule(
        &node,
        &p,
        ws,
        "doomed",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    assert_eq!(
        flows_get(&node, &p, ws, "schedule:doomed").await["id"],
        json!("schedule:doomed"),
        "the managed flow exists before the delete"
    );

    let out = call_tool(
        &node,
        &p,
        ws,
        "rules.delete",
        &json!({ "id": "doomed" }).to_string(),
    )
    .await
    .expect("rules.delete ok");
    let out: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(out["ok"], json!(true));
    assert_eq!(
        out["schedule"]["managed"],
        json!(false),
        "the delete reports the schedule as torn down"
    );

    let gone = call_tool(
        &node,
        &p,
        ws,
        "flows.get",
        &json!({ "id": "schedule:doomed" }).to_string(),
    )
    .await;
    assert!(
        gone.is_err(),
        "deleting the rule deletes its managed cron flow — an orphaned cron fires forever at a rule \
         that no longer exists (lb#167 follow-up)"
    );

    // …and the reactor has nothing left to fire: the effect, not just the record.
    let reactor = Principal::routed("node:reactor", ws, reactor_caps());
    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let pass = react_to_flows_cron(&node, &reactor, ws, u64::MAX / 2)
        .await
        .unwrap();
    assert_eq!(
        pass.fired, 0,
        "a torn-down schedule fires nothing, even at a far-future tick"
    );

    // Idempotent: a second delete (and a delete of an absent rule) is a no-op, not an error.
    call_tool(
        &node,
        &p,
        ws,
        "rules.delete",
        &json!({ "id": "doomed" }).to_string(),
    )
    .await
    .expect("a repeated rules.delete is a no-op");
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

/// Items on the inbox's `rules` channel — where `route_alerts` lands an `alert()` finding.
async fn count_inbox(node: &Arc<HostNode>, p: &Principal, ws: &str) -> usize {
    lb_host::list_inbox(&node.store, p, ws, "rules")
        .await
        .unwrap()
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

/// **A scheduled rule runs as its OWNER, not as the reactor** (lb#167, third verb deep).
///
/// The two tests above fix the fires that die on `rules.eval` / `inbox.record` — verbs it was right
/// to hand the reactor, because they are the mechanics of running a rule at all. This one covers the
/// class that CANNOT be fixed that way: a rule body whose first statement is
/// `query("<datasource>", ...)`, which collects via `federation.query`. That cap is deliberately
/// absent from `reactor_caps()` (granting it would give every scheduled flow on the node blanket
/// read access to every registered datasource), so the fire is `denied` while `rules.run` from the
/// UI succeeds under the user's own token — the lb#167 asymmetry, unfixable by widening.
///
/// The fix is run-as-owner: the fire executes as `SavedRule::scheduled_by` with that subject's caps
/// re-resolved LIVE from the grant store. So the assertion here is deliberately about a cap the
/// reactor does not have and the owner does — if the substitution stops happening, the rule is
/// denied and this fails.
///
/// Fails before `execute_node::run_as_owner`; passes after.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn scheduled_rule_runs_as_its_owner_with_caps_the_reactor_lacks() {
    use lb_authz::{grant_assign, Subject};

    let ws = "sched-run-as-owner";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    // The owner's authority must come from the GRANT STORE, not from the author's minted token:
    // run-as-owner re-resolves caps live on every fire, precisely so a demoted author's schedule
    // loses reach. Seeding a real grant row is what makes this test exercise that resolve.
    let owner = Subject::User("test".into());
    for cap in [
        "mcp:rules.eval:call",
        "mcp:insight.raise:call",
        "store:rule:read",
        "store:insight:write",
        // The cap that makes this test mean something: the reactor does NOT have it.
        FEDERATION_QUERY,
    ] {
        grant_assign(&node.store, ws, &owner, cap)
            .await
            .expect("grant assigned");
    }

    save_rule(
        &node,
        &author,
        ws,
        "owned",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // The owner is captured at save, on the record — the identity the fire resolves.
    let saved = get_rule(&node, &author, ws, "owned").await;
    assert_eq!(
        saved["scheduled_by"],
        json!("user:test"),
        "rules.save recorded the scheduling subject as the rule's owner"
    );

    // Fire under the REAL reactor principal, exactly as `spawn_flow_reactors` mints it.
    let reactor = Principal::routed("node:reactor", ws, reactor_caps());
    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:owned", "trigger").await;
    let pass = react_to_flows_cron(&node, &reactor, ws, next + 1)
        .await
        .unwrap();
    assert_eq!(pass.fired, 1, "the managed cron flow fired exactly one run");

    // THE MECHANISM, asserted directly. A body-level effect cannot prove this on its own: every verb
    // the cage can reach without a live datasource sidecar is one `reactor_caps()` already grants, so
    // such a test passes with the substitution ripped out (it was written that way first, and did).
    // What actually distinguishes fixed from broken is WHICH principal the rule node dispatches under.
    let reactor_only = Principal::routed("node:reactor", ws, reactor_caps());
    assert!(
        !reactor_only.caps().iter().any(|c| c == FEDERATION_QUERY),
        "precondition: `reactor_caps()` must NOT grant {FEDERATION_QUERY} — if it ever does, the \
         blanket-datasource-reach boundary was crossed and run-as-owner is no longer what carries \
         a scheduled rule to its datasource"
    );
    let swapped = owner_principal(&node, &reactor_only, ws, "owned")
        .await
        .expect("a headless fire of an owned rule substitutes the owner's principal");
    assert_eq!(
        swapped.sub(),
        "user:test",
        "the fire runs AS the owner, not as node:reactor"
    );
    assert_eq!(swapped.ws(), ws, "the owner principal is workspace-pinned");
    assert!(
        swapped.caps().iter().any(|c| c == FEDERATION_QUERY),
        "the owner's live grant carried {FEDERATION_QUERY} into the fire — this is the cap the \
         reactor lacks and the whole reason a `query(<datasource>, ..)` rule was denied on every \
         scheduled fire (lb#167)"
    );

    let run_id = cron_run_id("schedule:owned", "trigger", next);
    poll_run_terminal(&node, &author, ws, &run_id).await;
    assert_eq!(
        count_insights(&node, &author, ws).await,
        1,
        "the scheduled rule ran to its effect"
    );
}

/// **The substitution is actually WIRED into the `rule` node** — not merely available.
///
/// The test above proves `owner_principal` resolves the right authority; it cannot prove that
/// `core::rule` *uses* it (ripping the two-line swap out of `core.rs` leaves that test green — which
/// it did, verified). This one closes that gap the only way that is honest: fire the managed flow
/// under a reactor principal with `mcp:rules.eval:call` REMOVED, so the run can only succeed if the
/// node dispatched under the owner's caps instead of the ones it was handed.
///
/// That is a faithful stand-in for the production failure. On a live node the missing cap is
/// `mcp:federation.query:call` (needed by a `query("<datasource>", ..)` body and deliberately not in
/// `reactor_caps()`), which cannot be exercised here without a registered datasource + sidecar. The
/// mechanism under test is identical: a cap the reactor lacks and the owner holds.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_rule_node_dispatches_under_the_owner_not_the_principal_it_was_handed() {
    use lb_authz::{grant_assign, Subject};

    let ws = "sched-owner-wired";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    let owner = Subject::User("test".into());
    for cap in [
        "mcp:rules.eval:call",
        "mcp:insight.raise:call",
        "store:rule:read",
        "store:insight:write",
    ] {
        grant_assign(&node.store, ws, &owner, cap)
            .await
            .expect("grant assigned");
    }

    save_rule(
        &node,
        &author,
        ws,
        "owned",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // A CRIPPLED reactor: everything it normally has except the one verb the rule node needs. Without
    // run-as-owner wired in, the fire is `denied` and no insight is ever raised.
    let crippled: Vec<String> = reactor_caps()
        .into_iter()
        .filter(|c| c != "mcp:rules.eval:call")
        .collect();
    let reactor = Principal::routed("node:reactor", ws, crippled);

    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:owned", "trigger").await;
    react_to_flows_cron(&node, &reactor, ws, next + 1)
        .await
        .unwrap();

    let run_id = cron_run_id("schedule:owned", "trigger", next);
    poll_run_terminal(&node, &author, ws, &run_id).await;

    let run = call_tool(
        &node,
        &author,
        ws,
        "flows.runs.get",
        &json!({ "run_id": run_id }).to_string(),
    )
    .await
    .expect("flows.runs.get ok");
    let run: Value = serde_json::from_str(&run).unwrap();
    let rule_step = run["steps"]
        .as_array()
        .expect("steps")
        .iter()
        .find(|s| s["id"] == "rule")
        .expect("the managed flow has a rule step")
        .clone();
    assert_ne!(
        rule_step["error"],
        json!("denied"),
        "the rule node was DENIED — `core::rule` did not dispatch under the owner's principal, so \
         run-as-owner is resolvable but NOT wired in"
    );
    assert_eq!(run["status"], json!("success"), "no partialFailure");
    assert_eq!(
        count_insights(&node, &author, ws).await,
        1,
        "the fire reached its effect on a cap ONLY the owner held"
    );
}

/// **A manual run is never substituted.** Run-as-owner keys strictly off the reactor subject, so a
/// user-driven run keeps the caller's own authority — the substitution can neither widen a weak
/// caller nor narrow a strong one on a path nobody asked about.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_user_driven_run_keeps_its_own_principal() {
    let ws = "sched-manual-untouched";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);
    save_rule(
        &node,
        &author,
        ws,
        "owned",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;
    assert!(
        owner_principal(&node, &author, ws, "owned").await.is_none(),
        "a run under the author's own principal must not be swapped for the owner's"
    );
}

/// **Run-as-owner is fail-CLOSED**: no recorded owner ⇒ the reactor principal stands, unchanged.
///
/// The substitution must never become a way to acquire authority. A rule with no `scheduled_by` (one
/// saved before this shipped, or whose owner was cleared) has to keep behaving exactly as it did —
/// running on `reactor_caps()` alone — rather than falling back to anything wider. This pins the
/// degrade direction so a later refactor cannot quietly invert it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_scheduled_rule_without_a_recorded_owner_keeps_the_reactor_principal() {
    let ws = "sched-owner-absent";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    save_rule(
        &node,
        &author,
        ws,
        "unowned",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // Strip the owner, simulating a record written before run-as-owner existed.
    let raw = lb_store::read(&node.store, ws, "rule", "unowned")
        .await
        .unwrap()
        .expect("the saved rule");
    let mut rule: Value = raw;
    rule.as_object_mut().unwrap().remove("scheduled_by");
    lb_store::write(&node.store, ws, "rule", "unowned", &rule)
        .await
        .unwrap();

    // It still fires — on `reactor_caps()`, which since lb#167 does carry `rules.eval` + raise. The
    // point is that it neither errors nor gains anything: identical to the pre-run-as-owner path.
    let reactor = Principal::routed("node:reactor", ws, reactor_caps());
    react_to_flows_cron(&node, &reactor, ws, 100).await.unwrap();
    let next = cursor_next(&node, ws, "schedule:unowned", "trigger").await;
    react_to_flows_cron(&node, &reactor, ws, next + 1)
        .await
        .unwrap();

    let run_id = cron_run_id("schedule:unowned", "trigger", next);
    poll_run_terminal(&node, &author, ws, &run_id).await;
    assert_eq!(
        count_insights(&node, &author, ws).await,
        1,
        "an ownerless scheduled rule still runs on the reactor's own caps (no regression)"
    );
}

/// **`rules.adopt_schedule` claims an ownerless schedule** — the migration path for rules saved
/// before run-as-owner existed.
///
/// Those rules carry no `scheduled_by`, so they keep firing on `reactor_caps()` alone and stay broken
/// exactly as lb#167 describes. There is deliberately NO silent backfill: stamping a guessed subject
/// onto every ownerless rule would be a privilege grant made by a migration rather than by a person,
/// and the store holds no record of who authored a rule before the field existed. Adoption is
/// therefore an explicit act that can only ever confer the caller's OWN authority.
///
/// The `call_tool` path is load-bearing here, not incidental: it exercises the OUTER cap gate. A verb
/// with no alias derives `mcp:rules.adopt_schedule:call`, a cap that exists in no role bundle — so
/// without the `tool_gate` arm this verb would answer a bare `denied` for every caller, admins
/// included, while its own body's check passed. That failure is invisible to a test that calls the
/// host fn directly, and it has shipped twice before (`outbox.enqueue_held`, `media.upload_*`).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn adopt_schedule_records_the_caller_as_owner_through_the_cap_gate() {
    use lb_authz::{grant_assign, Subject};

    let ws = "sched-adopt";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);

    // The owner's authority comes from the GRANT STORE — adoption records an identity, and the fire
    // resolves that identity's caps live. Without a grant row the adopted owner resolves to nothing
    // and run-as-owner correctly declines to substitute (the fail-closed path).
    grant_assign(
        &node.store,
        ws,
        &Subject::User("test".into()),
        "mcp:rules.eval:call",
    )
    .await
    .expect("grant assigned");

    save_rule(
        &node,
        &author,
        ws,
        "legacy",
        &scheduled_rule_body("#[schedule(\"every 15 minutes\")]"),
    )
    .await;

    // Strip the owner — this IS the pre-run-as-owner record shape.
    let mut rule = lb_store::read(&node.store, ws, "rule", "legacy")
        .await
        .unwrap()
        .expect("the saved rule");
    rule.as_object_mut().unwrap().remove("scheduled_by");
    lb_store::write(&node.store, ws, "rule", "legacy", &rule)
        .await
        .unwrap();
    assert!(
        owner_principal(
            &node,
            &Principal::routed("node:reactor", ws, reactor_caps()),
            ws,
            "legacy"
        )
        .await
        .is_none(),
        "precondition: the stripped rule has no owner to run as"
    );

    let out = call_tool(
        &node,
        &author,
        ws,
        "rules.adopt_schedule",
        &json!({ "id": "legacy" }).to_string(),
    )
    .await
    .expect("rules.adopt_schedule passes the cap gate under `mcp:rules.save:call`");
    let out: Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        out["scheduled_by"],
        json!("user:test"),
        "the caller owns it"
    );

    // …and the fire now resolves that owner's live caps.
    let swapped = owner_principal(
        &node,
        &Principal::routed("node:reactor", ws, reactor_caps()),
        ws,
        "legacy",
    )
    .await;
    assert!(
        swapped.is_some(),
        "after adoption the scheduled fire runs as the owner"
    );
}

/// Adopting a rule with no `#[schedule(...)]` directive is refused: an unscheduled rule has no
/// headless fire, so an owner on it would be an identity nothing ever reads.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn adopt_schedule_refuses_an_unscheduled_rule() {
    let ws = "sched-adopt-unscheduled";
    let node = Arc::new(HostNode::boot().await.unwrap());
    let author = principal(ws, FULL);
    save_rule(
        &node,
        &author,
        ws,
        "ondemand",
        "insight.raise(#{ dedup_key: \"x\", severity: \"warning\", title: \"t\", body: #{} });",
    )
    .await;
    assert!(
        call_tool(
            &node,
            &author,
            ws,
            "rules.adopt_schedule",
            &json!({ "id": "ondemand" }).to_string(),
        )
        .await
        .is_err(),
        "an unscheduled rule has no scheduled fire to own"
    );
}
