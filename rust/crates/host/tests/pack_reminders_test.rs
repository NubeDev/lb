//! A pack seeds its own SCHEDULE — the `reminders:` block (pack-core-scope §Goals).
//!
//! **Why this exists.** Every other pack section seeds a thing that sits inert until something
//! drives it: a rule does nothing until `rules.run`, a channel until someone posts. A pack that
//! ships seven FDD rules and no schedule ships a product that detects nothing until an operator
//! wires the cron by hand — off-manifest, undocumented, and outside the receipt. That is exactly
//! what the rubix-ai BMS duty cycle had to do in a shell script. This block closes it.
//!
//! Real node (`mem://`), real caps, real seams — an applied reminder here goes through the very
//! `reminder.create` the public verb calls, and is read back through `reminder.list`. No mocks.
//!
//! Mandatory categories: the seam applies (and the reminder is really schedulable), the per-object
//! capability-deny (a pack must not smuggle a schedule past `mcp:reminder.create:call`), LWW on
//! re-apply, and the validate lints that gate a bad block before it ever reaches the node.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};

// ----- principals ---------------------------------------------------------------------------------

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
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

const PACK_SURFACE: &[&str] = &[
    "mcp:pack.validate:call",
    "mcp:pack.apply:call",
    "mcp:pack.list:call",
    "mcp:pack.get:call",
];

/// The pack surface + the reminder verbs the objects re-check + the channel gate the pack's other
/// object needs.
fn full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "mcp:reminder.create:call",
        "mcp:reminder.list:call",
        "bus:chan/*:pub",
        "bus:chan/*:sub",
    ]);
    principal(ws, &caps)
}

/// The partial-apply principal: everything EXCEPT `mcp:reminder.create:call`. The channel object
/// applies; the reminder object is denied. That asymmetry is the point — holding `pack.apply` must
/// not smuggle a schedule past the reminder verb's own gate.
fn missing_reminder_cap(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&["mcp:reminder.list:call", "bus:chan/*:pub", "bus:chan/*:sub"]);
    principal(ws, &caps)
}

// ----- the bundle ---------------------------------------------------------------------------------

/// A pack with one channel and two reminders — a `rules.run` sweep and an `agent.invoke` review.
/// The review carries `{{fire_ts}}`, which is what a real duty cycle needs and what the lint warns
/// about when it is missing.
fn bundle(version: u32, schedule: &str) -> Value {
    let manifest = format!(
        "pack: duty\ntitle: Duty Pack\nversion: {version}\n\
         channels:\n  - name: duty-alerts\n\
         reminders:\n\
         \x20 - id: duty-fast-flatline\n\
         \x20   schedule: \"{schedule}\"\n\
         \x20   action_kind: mcp-tool\n\
         \x20   tool: rules.run\n\
         \x20   args: {{ rule_id: fdd-sensor-flatline }}\n\
         \x20 - id: duty-daily-review\n\
         \x20   schedule: \"0 6 * * *\"\n\
         \x20   action_kind: mcp-tool\n\
         \x20   tool: agent.invoke\n\
         \x20   args: {{ job_id: \"daily-review-{{{{fire_ts}}}}\", goal: \"review the open insights\" }}\n"
    );
    json!({ "manifest": manifest, "files": {} })
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap_or(Value::Null))
}

async fn apply(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    b: Value,
    ts: u64,
) -> Result<Value, lb_mcp::ToolError> {
    call(node, p, ws, "pack.apply", json!({"bundle": b, "ts": ts})).await
}

/// Every reminder object's outcome, by id.
fn reminder_outcome(resp: &Value, id: &str) -> String {
    resp["objects"]
        .as_array()
        .expect("objects array")
        .iter()
        .find(|o| o["kind"] == "reminder" && o["id"] == id)
        .unwrap_or_else(|| panic!("no reminder object '{id}' in {resp}"))["outcome"]
        .as_str()
        .expect("outcome string")
        .to_string()
}

async fn listed(node: &Arc<Node>, p: &Principal, ws: &str) -> Vec<Value> {
    let out = call(node, p, ws, "reminder.list", json!({}))
        .await
        .expect("reminder.list");
    out["reminders"].as_array().cloned().unwrap_or_default()
}

// ----- 1. the headline: a pack really seeds a working schedule ------------------------------------

/// The applied reminder is a REAL reminder — present in `reminder.list`, with the manifest's cron
/// and action, and a `next_attempt_ts` strictly in the future (i.e. `reminder.create` computed a
/// first slot, so the reactor will actually pick it up). Asserting the row exists is not enough:
/// a reminder that never schedules a next attempt is inert, which is the failure this closes.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pack_seeds_a_schedule_that_will_actually_fire() {
    let ws = "ws-pack-reminders-headline";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = full(ws);
    let ts = 1_700_000_000;

    let out = apply(&node, &p, ws, bundle(1, "*/15 * * * *"), ts)
        .await
        .expect("apply");

    assert_eq!(reminder_outcome(&out, "duty-fast-flatline"), "applied");
    assert_eq!(reminder_outcome(&out, "duty-daily-review"), "applied");

    let rows = listed(&node, &p, ws).await;
    let fast = rows
        .iter()
        .find(|r| r["id"] == "duty-fast-flatline")
        .expect("the seeded reminder is a real, listable reminder");

    assert_eq!(fast["schedule"], "*/15 * * * *");
    assert_eq!(fast["action"]["tool"], "rules.run");
    assert_eq!(fast["action"]["args"]["rule_id"], "fdd-sensor-flatline");
    // `nextAttemptTs` (camelCase on the wire) is the proof the reminder is SCHEDULED and not merely
    // stored: `reminder.create` computed the next slot strictly after the apply clock, which is what
    // the reactor's due-scan picks up. A row with no next attempt is an inert reminder.
    assert!(
        fast["nextAttemptTs"].as_u64().expect("a next attempt") > ts,
        "the reminder is SCHEDULED, not just stored: {fast}"
    );

    // The `{{fire_ts}}` token survives into the stored args VERBATIM — substitution happens at fire
    // time, not apply time. Baking the apply clock in here would freeze every firing's job_id to the
    // apply moment, which is the exact replay bug this token exists to prevent.
    let review = rows
        .iter()
        .find(|r| r["id"] == "duty-daily-review")
        .expect("the review reminder");
    assert_eq!(
        review["action"]["args"]["job_id"], "daily-review-{{fire_ts}}",
        "the placeholder is resolved at FIRE time, so it must be stored unsubstituted: {review}"
    );
}

// ----- 2. the per-object capability wall ---------------------------------------------------------

/// **No cap smuggling.** `mcp:pack.apply:call` gets a caller into the orchestration and nothing
/// more. Without `mcp:reminder.create:call` the reminder objects are DENIED — while the channel in
/// the same pack still applies, so this is an honest partial, not an abort — and nothing is
/// scheduled. A pack must never be a back door onto a schedule.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_pack_cannot_smuggle_a_schedule_past_the_reminder_gate() {
    let ws = "ws-pack-reminders-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let short = missing_reminder_cap(ws);

    let out = apply(&node, &short, ws, bundle(1, "*/15 * * * *"), 1_700_000_000)
        .await
        .expect("the apply itself is allowed — it is the OBJECT that is denied");

    assert_eq!(reminder_outcome(&out, "duty-fast-flatline"), "denied");
    assert_eq!(reminder_outcome(&out, "duty-daily-review"), "denied");
    // The rest of the pack still applied — a partial, not an abort.
    let channel = out["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["kind"] == "channel")
        .expect("the channel object");
    assert_eq!(channel["outcome"], "applied");

    // And nothing was scheduled. Read back with a principal that CAN list, so an empty result means
    // "nothing there", never "you cannot see it".
    assert!(
        listed(&node, &full(ws), ws).await.is_empty(),
        "a denied reminder object must leave NO reminder behind"
    );
}

// ----- 3. LWW on re-apply -------------------------------------------------------------------------

/// A reminder is an LWW upsert keyed by `id`, like every other inline pack object. Editing the cron
/// and re-applying at a new version must MOVE the existing schedule, never fork a second reminder
/// that fires the same rule twice.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_applying_an_edited_schedule_moves_it_rather_than_forking_it() {
    let ws = "ws-pack-reminders-lww";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = full(ws);

    apply(&node, &p, ws, bundle(1, "*/15 * * * *"), 1_700_000_000)
        .await
        .expect("first apply");
    let out = apply(&node, &p, ws, bundle(2, "*/30 * * * *"), 1_700_000_100)
        .await
        .expect("re-apply at a new version");
    assert_eq!(reminder_outcome(&out, "duty-fast-flatline"), "applied");

    let rows = listed(&node, &p, ws).await;
    assert_eq!(
        rows.iter()
            .filter(|r| r["id"] == "duty-fast-flatline")
            .count(),
        1,
        "the edit UPSERTS the same id — two rows would fire the sweep twice: {rows:?}"
    );
    assert_eq!(
        rows.iter()
            .find(|r| r["id"] == "duty-fast-flatline")
            .unwrap()["schedule"],
        "*/30 * * * *",
        "and the new cron is the one that stuck"
    );
}

// ----- 4. the lints gate a bad block BEFORE it reaches the node -----------------------------------

/// An unknown `action_kind` is a validate ERROR, so the apply never runs. The manifest holds the
/// kind as a `String` (it is a dependency-free mirror of the verb args), so without this lint the
/// typo would reach the converter and apply as the wrong action entirely.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_unknown_action_kind_is_refused_at_validate() {
    let ws = "ws-pack-reminders-lint";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = full(ws);

    let bad = json!({
        "manifest": "pack: duty\ntitle: T\nversion: 1\n\
                     reminders:\n  - id: r1\n    schedule: \"* * * * *\"\n    action_kind: mcp_tool\n",
        "files": {}
    });

    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": bad.clone()}),
    )
    .await
    .expect("validate answers");
    let findings = serde_json::to_string(&out).unwrap();
    assert!(
        findings.contains("unknown action_kind"),
        "validate names the bad kind: {findings}"
    );

    // And an errored validate GATES the apply — nothing is scheduled.
    let _ = apply(&node, &p, ws, bad, 1_700_000_000).await;
    assert!(
        listed(&node, &p, ws).await.is_empty(),
        "an errored lint must stop the apply before any reminder is created"
    );
}

/// **The replay trap, surfaced as a warning.** A static `job_id` on a verb idempotent on it returns
/// the FIRST run's answer on every firing — a frozen agent that looks alive. A warning, not an
/// error (the manifest cannot know which verbs are idempotent on which field), so the apply
/// proceeds; what matters is that the author is TOLD.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_static_job_id_warns_but_does_not_block() {
    let ws = "ws-pack-reminders-replay";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = full(ws);

    let static_id = json!({
        "manifest": "pack: duty\ntitle: T\nversion: 1\n\
                     reminders:\n  - id: r1\n    schedule: \"0 6 * * *\"\n\
                     \x20   action_kind: mcp-tool\n    tool: agent.invoke\n\
                     \x20   args: { job_id: daily-review, goal: g }\n",
        "files": {}
    });

    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": static_id.clone()}),
    )
    .await
    .expect("validate answers");
    let findings = serde_json::to_string(&out).unwrap();
    assert!(
        findings.contains("fire_ts"),
        "the warning names the fix: {findings}"
    );

    // A warning does not gate: the reminder still applies.
    let applied = apply(&node, &p, ws, static_id, 1_700_000_000)
        .await
        .expect("apply");
    assert_eq!(reminder_outcome(&applied, "r1"), "applied");
}
