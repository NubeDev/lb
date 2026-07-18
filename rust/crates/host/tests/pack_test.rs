//! Host-layer tests for the `pack.*` verb family (pack-core-scope §Tests, the house bar). Real node
//! (`mem://` store), real caps, real seams — a pack applied here drives the very `rules.save` /
//! `dashboard.save` / `channel.create` / `agent.memory.set` functions the public verbs call. NO
//! mocks: the pure half is unit-tested in `lb-packs`, and what is left to prove is exactly the part
//! that only a real node can answer — that the caps wall re-fires per object, that the refusal
//! matrix holds against a real receipt, and that the workspace wall is physical.
//!
//! Mandatory categories: capability-deny (every verb), the refusal matrix end-to-end, the cap-deny →
//! partial → grant → re-apply RECOVERY (the scope's named test), the workspace wall (two packs, two
//! workspaces, no cross-reads), and the loud clobber listing. The demo oracle — blank node → one
//! apply → a real insight raises — rides at the bottom behind `#[ignore]`, since it needs the real
//! federation sidecar built.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, insight_list, Node};
use lb_insights::ListQuery;
use serde_json::{json, Value};

// ----- the bas fixture, as a bundle over the wire -----------------------------------------------
//
// `include_str!` rather than a filesystem read at runtime: the bundle IS the payload a caller sends,
// so the test ships the bytes the same way a third party would (pack-core-scope
// §"Bundle-over-the-wire" — applying needs a session and caps, never a node filesystem).

const BAS_MANIFEST: &str = include_str!("fixtures/packs/bas/pack.yaml");
const BAS_SCHEMA: &str = include_str!("fixtures/packs/bas/schema.sql");
const BAS_SEED: &str = include_str!("fixtures/packs/bas/seed.sql");
const BAS_AGENT: &str = include_str!("fixtures/packs/bas/agent-context.md");
const BAS_DASHBOARD: &str = include_str!("fixtures/packs/bas/dashboards/plant-overview.json");

const BAS_RULES: &[(&str, &str)] = &[
    (
        "rules/fdd-sensor-flatline.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-sensor-flatline.rhai"),
    ),
    (
        "rules/fdd-cooling-failure.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-cooling-failure.rhai"),
    ),
    (
        "rules/fdd-short-cycling.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-short-cycling.rhai"),
    ),
    (
        "rules/fdd-after-hours.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-after-hours.rhai"),
    ),
    (
        "rules/fdd-night-water.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-night-water.rhai"),
    ),
    (
        "rules/fdd-energy-drift.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-energy-drift.rhai"),
    ),
    (
        "rules/fdd-energy-intensity.rhai",
        include_str!("fixtures/packs/bas/rules/fdd-energy-intensity.rhai"),
    ),
];

/// The bas bundle exactly as a caller would send it — manifest text plus every referenced file.
fn bas_bundle() -> Value {
    let mut files = serde_json::Map::new();
    files.insert("schema.sql".into(), json!(BAS_SCHEMA));
    files.insert("seed.sql".into(), json!(BAS_SEED));
    files.insert("agent-context.md".into(), json!(BAS_AGENT));
    files.insert(
        "dashboards/plant-overview.json".into(),
        json!(BAS_DASHBOARD),
    );
    for (path, body) in BAS_RULES {
        files.insert((*path).into(), json!(body));
    }
    json!({"manifest": BAS_MANIFEST, "files": Value::Object(files)})
}

// ----- the small inline pack ---------------------------------------------------------------------
//
// The refusal-matrix and recovery tests use this rather than bas on purpose: one channel + one agent
// context is enough to exercise every row of the matrix and both sides of the per-object caps wall,
// with no datasource — so no sqlite materialization and no federation sidecar. Fast, and it isolates
// what the test is actually about.

const SMALL_CONTEXT: &str = "# Small pack context\n\nThe authored domain facts.\n";

/// A minimal bundle: one channel, one agent context. `version` and `context` vary so the tests can
/// drive the "same version, changed file" and version-skew rows of the matrix.
fn small_bundle(version: u32, context: &str) -> Value {
    let manifest = format!(
        "pack: small\ntitle: Small Pack\nversion: {version}\n\
         channels:\n  - name: small-alerts\n\
         agent:\n  context: ctx.md\n"
    );
    json!({"manifest": manifest, "files": {"ctx.md": context}})
}

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

/// The `pack.*` surface caps only — enough to enter the verbs, and deliberately nothing downstream.
/// Holding these must NOT smuggle a caller past any object's own gate.
const PACK_SURFACE: &[&str] = &[
    "mcp:pack.validate:call",
    "mcp:pack.apply:call",
    "mcp:pack.list:call",
    "mcp:pack.get:call",
];

/// Everything the SMALL pack needs: the pack surface plus the per-object caps its two objects
/// re-check — the channel `pub` gate and the agent-memory verb + its distinct workspace-scope write.
fn small_full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:agent.memory.set:call",
        "store:agent_memory/workspace:write",
    ]);
    principal(ws, &caps)
}

/// The partial-apply principal: the pack surface + the channel gate + the agent-memory VERB, but NOT
/// the workspace-scope memory write. The channel object applies; the agent object is denied. That
/// asymmetry is the whole point — one grant short produces a partial, not an abort and not a silent
/// success.
fn small_missing_agent_cap(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:agent.memory.set:call",
    ]);
    principal(ws, &caps)
}

// ----- helpers ------------------------------------------------------------------------------------

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
    bundle: Value,
    ts: u64,
) -> Result<Value, lb_mcp::ToolError> {
    call(
        node,
        p,
        ws,
        "pack.apply",
        json!({"bundle": bundle, "ts": ts}),
    )
    .await
}

/// The outcome recorded for one object kind in an apply response.
fn object_outcome(resp: &Value, kind: &str) -> String {
    resp["objects"]
        .as_array()
        .expect("objects array")
        .iter()
        .find(|o| o["kind"] == kind)
        .unwrap_or_else(|| panic!("no {kind} object in {resp}"))["outcome"]
        .as_str()
        .expect("outcome string")
        .to_string()
}

// ----- 1. capability-deny --------------------------------------------------------------------------

/// Every `pack.*` verb refuses a principal holding no caps. The gate fires on the SURFACE, before a
/// bundle is parsed or a receipt is read — a caller with no grant learns nothing about the workspace.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn each_pack_verb_is_denied_without_its_cap() {
    let ws = "pack-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let nobody = principal(ws, &[]);

    for (tool, input) in [
        (
            "pack.validate",
            json!({"bundle": small_bundle(1, SMALL_CONTEXT)}),
        ),
        (
            "pack.apply",
            json!({"bundle": small_bundle(1, SMALL_CONTEXT), "ts": 1}),
        ),
        ("pack.list", json!({})),
        ("pack.get", json!({"pack": "small"})),
    ] {
        let err = call(&node, &nobody, ws, tool, input)
            .await
            .expect_err("{tool} must be denied without its cap");
        assert!(
            matches!(err, lb_mcp::ToolError::Denied),
            "{tool} refused with {err:?}, expected Denied"
        );
    }
}

// ----- 2. validate: the plan the apply will follow ---------------------------------------------------

/// `pack.validate` on the real bas fixture is the CI validator's answer: the pack is clean, a blank
/// workspace would apply it, and the plan is EXACTLY the objects the manifest describes — one
/// datasource, seven rules, one dashboard, one channel, one agent context. The plan is the spine the
/// dry run, the apply, and the receipt all derive from, so pinning it here pins all three.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn validate_of_the_bas_fixture_plans_every_object() {
    let ws = "pack-validate";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": bas_bundle()}),
    )
    .await
    .expect("pack.validate on the bas fixture");

    assert_eq!(
        out["valid"], true,
        "the shipped fixture must lint clean: {out}"
    );
    assert_eq!(
        out["decision"], "apply",
        "a blank workspace has no receipt, so the decision is a first apply: {out}"
    );
    assert_eq!(out["pack"], "bas");

    let plan = out["plan"].as_array().expect("plan array");
    assert_eq!(
        plan.len(),
        11,
        "1 datasource + 7 rules + 1 dashboard + 1 channel + 1 agent: {out}"
    );

    let kind_count = |k: &str| plan.iter().filter(|o| o["kind"] == k).count();
    assert_eq!(kind_count("datasource"), 1);
    assert_eq!(kind_count("rule"), 7);
    assert_eq!(kind_count("dashboard"), 1);
    assert_eq!(kind_count("channel"), 1);
    assert_eq!(kind_count("agent"), 1);

    // The rules query this source BY NAME, so the registered id is part of the contract, not a detail.
    let ds = plan.iter().find(|o| o["kind"] == "datasource").unwrap();
    assert_eq!(
        ds["id"], "demo-buildings",
        "the datasource id the rules query: {out}"
    );
}

// ----- 3. validate: lint errors gate -----------------------------------------------------------------

/// A self-inconsistent pack reports `valid: false`. A dangling entity parent is the canonical case:
/// the vocabulary tree cannot be rendered, so the author must learn it from `pack.validate` in CI
/// rather than from a half-applied workspace.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn validate_reports_lint_errors_and_marks_the_pack_invalid() {
    let ws = "pack-lint";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    let bundle = json!({
        "manifest": "pack: broken\ntitle: Broken\nversion: 1\n\
                     entities:\n  point:\n    label: Point\n    parent: equip\n",
        "files": {},
    });

    let out = call(&node, &p, ws, "pack.validate", json!({"bundle": bundle}))
        .await
        .expect("validate runs — an invalid pack is a REPORT, not a transport error");

    assert_eq!(out["valid"], false, "a dangling parent must gate: {out}");
    let findings = out["findings"].as_array().expect("findings array");
    assert!(
        findings
            .iter()
            .any(|f| f["severity"] == "error" && f["message"].as_str().unwrap().contains("equip")),
        "the finding names the undeclared parent: {out}"
    );

    // And the gate is real: an apply of the same bundle refuses rather than half-applying.
    let err = apply(&node, &p, ws, bundle, 1)
        .await
        .expect_err("an invalid pack must not apply");
    assert!(
        matches!(&err, lb_mcp::ToolError::BadInput(m) if m.contains("invalid")),
        "expected a loud invalid-pack refusal, got {err:?}"
    );
}

// ----- 4. a missing referenced file is loud -----------------------------------------------------------

/// A manifest that names a file the bundle does not carry is a hard `BadInput`, never a silent skip.
/// A pack that ships seven rule paths and six rule files is BROKEN, and the author must learn that
/// loudly — a skipped object would apply a pack that silently is not the pack they described.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_missing_referenced_file_is_a_loud_bad_input() {
    let ws = "pack-missing";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    // The manifest names a rule; the bundle carries no such file.
    let bundle = json!({
        "manifest": "pack: gappy\ntitle: Gappy\nversion: 1\nrules: [rules/absent.rhai]\n",
        "files": {},
    });

    for tool in ["pack.validate", "pack.apply"] {
        let err = call(&node, &p, ws, tool, json!({"bundle": bundle, "ts": 1}))
            .await
            .expect_err("{tool} must refuse a bundle with a missing file");
        match err {
            lb_mcp::ToolError::BadInput(m) => assert!(
                m.contains("rules/absent.rhai"),
                "the error names the missing path: {m}"
            ),
            other => panic!("{tool} expected BadInput, got {other:?}"),
        }
    }
}

// ----- 5. the refusal matrix, end to end ---------------------------------------------------------------

/// The refusal matrix against a REAL node and a REAL receipt — every row that a re-apply can hit:
/// first apply → same bundle → changed file at the same version → higher version → lower version.
/// The pure `decide` is unit-tested in `lb-packs`; what this proves is that the receipt actually
/// written by an apply feeds the matrix correctly on the next call. That round trip is where an
/// idempotence bug would really live.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_refusal_matrix_holds_against_a_real_receipt() {
    let ws = "pack-matrix";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = small_full(ws);

    // ROW 1 — no prior receipt: apply everything, and run the rules (the only run that ever does).
    let first = apply(&node, &p, ws, small_bundle(1, SMALL_CONTEXT), 10)
        .await
        .expect("first apply");
    assert_eq!(first["outcome"], "applied", "first apply: {first}");
    assert_eq!(
        first["ran_rules"], true,
        "rules run on the first apply only: {first}"
    );

    // The receipt is written — that is what every subsequent row reads.
    let got = call(&node, &p, ws, "pack.get", json!({"pack": "small"}))
        .await
        .expect("the first apply wrote a receipt");
    assert_eq!(got["version"], 1);
    assert_eq!(got["objects"].as_array().unwrap().len(), 2);

    // ROW 2 — same version, same content: the idempotent no-op. Nothing is touched, and the caller
    // can tell "already applied" from "just applied".
    let again = apply(&node, &p, ws, small_bundle(1, SMALL_CONTEXT), 11)
        .await
        .expect("a re-apply of an identical bundle is not an error");
    assert_eq!(again["outcome"], "noop", "identical re-apply: {again}");

    // ROW 3 — changed file, same version: refuse, and say the word the author needs to act on.
    let err = apply(
        &node,
        &p,
        ws,
        small_bundle(1, "# Small pack context\n\nEDITED.\n"),
        12,
    )
    .await
    .expect_err("changed content at the same version must refuse");
    match err {
        lb_mcp::ToolError::BadInput(m) => assert!(
            m.contains("bump"),
            "the refusal must tell the author to bump the version: {m}"
        ),
        other => panic!("expected a BadInput refusal, got {other:?}"),
    }

    // ROW 4 — a higher version means an upgrade, which this engine does not do. Refused honestly
    // rather than silently re-applied as if a version bump were cosmetic.
    let err = apply(&node, &p, ws, small_bundle(2, SMALL_CONTEXT), 13)
        .await
        .expect_err("a higher version must refuse");
    assert!(
        matches!(&err, lb_mcp::ToolError::BadInput(m) if m.contains("HIGHER") || m.contains("upgrade")),
        "expected an upgrade-not-built refusal, got {err:?}"
    );

    // ROW 5 — a downgrade is always refused.
    let bump = apply(&node, &p, ws, small_bundle(1, SMALL_CONTEXT), 14).await;
    assert_eq!(
        bump.unwrap()["outcome"],
        "noop",
        "state is still at version 1"
    );
    let err = apply(&node, &p, ws, small_bundle(0, SMALL_CONTEXT), 15)
        .await
        .expect_err("a lower version must refuse");
    assert!(
        matches!(&err, lb_mcp::ToolError::BadInput(m) if m.contains("LOWER") || m.contains("downgrade")),
        "expected a downgrade refusal, got {err:?}"
    );
}

// ----- 6. cap-deny → partial → grant → re-apply RECOVERY -----------------------------------------------

/// **The scope's named test, and the most important one here.** It proves two invariants at once:
///
///   1. **No cap smuggling.** `mcp:pack.apply:call` gets a caller into the orchestration and nothing
///      more. A principal one grant short — it may set agent memory, but not in the shared WORKSPACE
///      scope — gets `denied` on exactly that object and `applied` on the channel beside it. Per-object
///      authority is re-checked under the caller's own principal, so a pack is not a privileged path.
///   2. **The partial recovers.** The partial receipt is written (it IS the recovery signal), and the
///      documented fix — grant the cap, re-run — actually works: the SAME bundle at the SAME version
///      re-applies instead of no-opping. A no-op here would strand the workspace half-configured
///      forever, which is why the matrix has a sixth row for a partial prior receipt.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_cap_denied_partial_recovers_when_the_cap_is_granted() {
    let ws = "pack-recovery";
    let node = Arc::new(Node::boot().await.unwrap());
    let short = small_missing_agent_cap(ws);
    let full = small_full(ws);

    // --- PARTIAL: one grant short, and the denial is scoped to the object that needed it ---
    let partial = apply(&node, &short, ws, small_bundle(1, SMALL_CONTEXT), 20)
        .await
        .expect("a cap-denied object is a partial outcome, NOT a transport error");

    assert_eq!(
        partial["outcome"], "partial",
        "one denied object makes the apply partial: {partial}"
    );
    assert_eq!(
        object_outcome(&partial, "agent"),
        "denied",
        "the agent context needs the workspace-scope memory write this principal lacks: {partial}"
    );
    assert_eq!(
        object_outcome(&partial, "channel"),
        "applied",
        "the object whose cap the caller DOES hold still applies — a partial is not an abort: {partial}"
    );

    // The receipt exists even though the apply was partial — without it, "grant the cap, re-run"
    // would have nothing to recover from.
    let receipt = call(&node, &short, ws, "pack.get", json!({"pack": "small"}))
        .await
        .expect("a partial apply still writes its receipt");
    assert_eq!(receipt["version"], 1);
    assert!(
        receipt["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|o| o["kind"] == "agent" && o["outcome"] == "denied"),
        "the receipt records WHICH object was denied: {receipt}"
    );

    // (The roster's view of a partial is asserted in `list_strips_the_manifest_and_get_carries_it`,
    // which owns the `pack.list` shape — this test stays about the caps wall and the recovery.)

    // --- RECOVERY: grant the cap, re-run the IDENTICAL bundle at the SAME version ---
    let recovered = apply(&node, &full, ws, small_bundle(1, SMALL_CONTEXT), 21)
        .await
        .expect("the recovery re-apply");

    assert_ne!(
        recovered["outcome"], "noop",
        "a partial prior receipt must NOT no-op — that would strand the recovery: {recovered}"
    );
    assert_eq!(
        recovered["outcome"], "applied",
        "with the cap granted, every object applies: {recovered}"
    );
    assert_eq!(
        object_outcome(&recovered, "agent"),
        "applied",
        "the previously-denied object is the one the recovery had to fix: {recovered}"
    );
    assert_eq!(
        recovered["ran_rules"], false,
        "a recovery re-apply must not re-run rules — idempotence cannot depend on a dedup key: \
         {recovered}"
    );

    // And once whole, the pack settles back into the idempotent no-op.
    let settled = apply(&node, &full, ws, small_bundle(1, SMALL_CONTEXT), 22)
        .await
        .expect("re-apply after recovery");
    assert_eq!(
        settled["outcome"], "noop",
        "a complete receipt at the same version is a no-op again: {settled}"
    );
}

// ----- 7. the workspace wall -----------------------------------------------------------------------------

/// Two different packs in two workspaces never see each other. The wall is the store's namespace, so
/// this is physical rather than a filter — but a filter is exactly what a regression would replace it
/// with, so the test asserts all three faces: a roster sees only its own, `pack.get` for the other
/// workspace's pack is `NotFound`, and a ws-A principal is refused against ws-B outright.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn packs_do_not_leak_across_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    let (ws_a, ws_b) = ("wall-a", "wall-b");
    let pa = small_full(ws_a);
    let pb = small_full(ws_b);

    // Two DIFFERENT packs — different ids, so a leak would be unmistakable.
    let pack_a = json!({
        "manifest": "pack: alpha\ntitle: Alpha\nversion: 1\nchannels:\n  - name: alpha-alerts\n",
        "files": {},
    });
    let pack_b = json!({
        "manifest": "pack: beta\ntitle: Beta\nversion: 1\nchannels:\n  - name: beta-alerts\n",
        "files": {},
    });

    assert_eq!(
        apply(&node, &pa, ws_a, pack_a, 30).await.unwrap()["outcome"],
        "applied"
    );
    assert_eq!(
        apply(&node, &pb, ws_b, pack_b, 31).await.unwrap()["outcome"],
        "applied"
    );

    // Each workspace reads its OWN pack, and the other's is not merely hidden — it is unreadable.
    for (p, ws, mine, theirs) in [(&pa, ws_a, "alpha", "beta"), (&pb, ws_b, "beta", "alpha")] {
        let got = call(&node, p, ws, "pack.get", json!({"pack": mine}))
            .await
            .expect("a workspace reads its own receipt");
        assert_eq!(got["pack"], mine, "{ws} reads its own pack: {got}");

        let err = call(&node, p, ws, "pack.get", json!({"pack": theirs}))
            .await
            .expect_err("a cross-workspace pack.get must not resolve");
        assert!(
            matches!(err, lb_mcp::ToolError::NotFound),
            "expected NotFound for {theirs} from {ws}, got {err:?}"
        );
    }

    // And the wall holds one level up: a ws-A token is refused against ws-B before any read happens.
    let err = call(&node, &pa, ws_b, "pack.list", json!({}))
        .await
        .expect_err("a ws-A principal must not read ws-B");
    assert!(
        matches!(err, lb_mcp::ToolError::Denied),
        "expected Denied across the workspace wall, got {err:?}"
    );
}

// ----- 8. the loud clobber listing -------------------------------------------------------------------------

/// A re-apply overwrites the pack's own objects, and every overwrite is named in the response
/// (`kind:id`). This is loud by contract: an admin who hand-tuned a dashboard or the agent context
/// learns exactly what the re-apply cost them. A silent clobber is the failure mode this prevents.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reapply_lists_every_object_it_clobbered() {
    let ws = "pack-clobber";
    let node = Arc::new(Node::boot().await.unwrap());
    let short = small_missing_agent_cap(ws);
    let full = small_full(ws);

    // A first apply clobbers nothing — there was nothing there.
    let first = apply(&node, &short, ws, small_bundle(1, SMALL_CONTEXT), 40)
        .await
        .expect("first apply");
    assert_eq!(first["outcome"], "partial");
    assert_eq!(
        first["clobbered"].as_array().unwrap().len(),
        0,
        "a first apply overwrites nothing: {first}"
    );

    // The recovery re-apply DOES overwrite the pack-owned objects, and says so — including the agent
    // context, the sharpest edge, which is never overwritten silently.
    let again = apply(&node, &full, ws, small_bundle(1, SMALL_CONTEXT), 41)
        .await
        .expect("recovery re-apply");
    let clobbered: Vec<&str> = again["clobbered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        clobbered.contains(&"channel:small-alerts"),
        "the channel overwrite is listed: {again}"
    );
    assert!(
        clobbered.contains(&"agent:small"),
        "the agent context overwrite is listed — the sharpest clobber edge: {again}"
    );
    assert_eq!(
        clobbered.len(),
        2,
        "every planned object is accounted for: {again}"
    );
}

// ----- 9. the read shapes ---------------------------------------------------------------------------------

/// `pack.list` and `pack.get` are the first-class receipt reads that replace a `store.query` on a
/// receipts table. The split is deliberate and worth pinning: the roster STRIPS the manifest (a list
/// read must not carry every pack's full vocabulary), and `pack.get` carries it so a reader can
/// render the pack — entities and all — without re-sending the bundle. The roster's `complete` flag
/// is what an operator scans to spot a partial apply, so it is pinned here too.
///
/// KNOWN FAILING against the current `pack.list` — see `scan_receipts`, which decodes the raw scan
/// row instead of the `write` envelope and silently drops every receipt. The assertions below encode
/// the intended contract, not today's behavior.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn list_strips_the_manifest_and_get_carries_it() {
    let ws = "pack-shapes";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = small_full(ws);

    // A pack with a real entity vocabulary — that is what must survive the round trip.
    let bundle = json!({
        "manifest": "pack: vocab\ntitle: Vocab Pack\nversion: 3\n\
                     entities:\n  site:\n    label: Site\n  \
                     equip:\n    label: Equipment\n    parent: site\n\
                     channels:\n  - name: vocab-alerts\n",
        "files": {},
    });
    apply(&node, &p, ws, bundle, 50).await.expect("apply");

    // --- list: the roster summary, no manifest ---
    let list = call(&node, &p, ws, "pack.list", json!({}))
        .await
        .expect("pack.list");
    let row = &list["packs"][0];
    assert_eq!(row["pack"], "vocab");
    assert_eq!(row["title"], "Vocab Pack");
    assert_eq!(row["version"], 3);
    assert_eq!(row["applied_ts"], 50);
    assert_eq!(row["complete"], true);
    assert_eq!(
        row["objects"], 1,
        "the roster carries a COUNT, not the objects"
    );
    assert!(
        row.get("manifest").is_none(),
        "a roster read must not carry the vocabulary: {list}"
    );

    // --- get: the full receipt, manifest included and round-tripped ---
    let got = call(&node, &p, ws, "pack.get", json!({"pack": "vocab"}))
        .await
        .expect("pack.get");
    assert_eq!(got["pack"], "vocab");
    assert_eq!(got["version"], 3);
    assert_eq!(
        got["manifest"]["entities"]["equip"]["parent"], "site",
        "the entity vocabulary round-trips so a reader can render it: {got}"
    );
    let objects = got["objects"].as_array().expect("objects");
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0]["kind"], "channel");
    assert_eq!(objects[0]["id"], "vocab-alerts");
    assert_eq!(objects[0]["outcome"], "applied");
    assert!(
        objects[0]["checksum"]
            .as_str()
            .is_some_and(|c| !c.is_empty()),
        "each object carries the checksum drift is measured by: {got}"
    );

    // A partial apply shows up in the roster as `complete: false` — the flag an operator scans for.
    let ws2 = "pack-shapes-partial";
    let short = small_missing_agent_cap(ws2);
    apply(&node, &short, ws2, small_bundle(1, SMALL_CONTEXT), 51)
        .await
        .expect("partial apply");
    let list2 = call(&node, &short, ws2, "pack.list", json!({}))
        .await
        .expect("pack.list");
    assert_eq!(
        list2["packs"][0]["complete"], false,
        "the roster shows a partially-applied pack is not complete: {list2}"
    );

    // An unknown pack is NotFound, not an empty record.
    let err = call(&node, &p, ws, "pack.get", json!({"pack": "nope"}))
        .await
        .expect_err("an unapplied pack has no receipt");
    assert!(matches!(err, lb_mcp::ToolError::NotFound), "got {err:?}");
}

// ----- 10. the demo oracle (ignored — needs the real federation sidecar) --------------------------------------

/// **The demo oracle** (pack-core-scope §Tests): a blank node, ONE `pack.apply` of the real `bas`
/// fixture, and a real FDD insight raises. This is the end-to-end claim the whole family exists to
/// make — "blank node + one call = a working product" — and nothing short of a real apply against a
/// real datasource can prove it.
///
/// `#[ignore]` because it is the one test here with an external prerequisite: the `bas` pack declares
/// a sqlite datasource, so the apply materializes a real `.db` and registers it through federation,
/// which needs the REAL `federation` sidecar binary spawned by the real supervisor. Building it costs
/// a full `cargo build -p federation`, so it does not ride the default `cargo test` run.
///
/// **How to run it:**
/// ```text
/// cd rust
/// cargo build -p federation                     # or set FEDERATION_BIN=/path/to/federation
/// cargo test -p lb-host --test pack_test -- --ignored --nocapture
/// ```
/// `LB_DIR` is pointed at a temp dir by the test itself, so the materialized pack db never lands in
/// the repo.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs the real federation sidecar built: cargo build -p federation"]
async fn the_demo_oracle_blank_node_one_apply_raises_a_real_insight() {
    use lb_host::install_native;
    use lb_supervisor::OsLauncher;

    const FEDERATION_MANIFEST: &str = include_str!("../../federation/extension.toml");

    // The materialized pack db is a node-local file; keep it out of the repo.
    let lb_dir = std::env::temp_dir().join(format!("lb-pack-oracle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lb_dir);
    std::env::set_var("LB_DIR", &lb_dir);

    // Build the sidecar exactly as `federation_sqlite_test.rs` does — default features, sqlite only,
    // no Docker and no TLS toolchain. A failure here is a FAIL, not a skip.
    let dir = {
        if let Ok(p) = std::env::var("FEDERATION_BIN") {
            std::path::PathBuf::from(&p)
                .parent()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        } else {
            let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let target = manifest_dir.join("../../target/debug");
            let status = std::process::Command::new("cargo")
                .args(["build", "-p", "federation"])
                .current_dir(manifest_dir.join("../.."))
                .status()
                .expect("cargo build -p federation runs");
            assert!(
                status.success() && target.join("federation").exists(),
                "the default-features (sqlite) federation sidecar builds"
            );
            target.to_string_lossy().into_owned()
        }
    };

    let ws = "oracle";
    let node = Arc::new(Node::boot().await.unwrap());

    // The full grant a real applying admin holds: the pack surface plus every per-object cap the bas
    // plan re-checks, plus what the rules need to query the source and raise.
    let admin = principal(
        ws,
        &[
            "mcp:pack.validate:call",
            "mcp:pack.apply:call",
            "mcp:pack.list:call",
            "mcp:pack.get:call",
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "mcp:federation.query:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
            "mcp:rules.save:call",
            "mcp:rules.run:call",
            "mcp:rules.get:call",
            "store:rule:write",
            "store:rule:read",
            "mcp:dashboard.save:call",
            "store:dashboard:write",
            "store:dashboard:read",
            "mcp:agent.memory.set:call",
            "store:agent_memory/workspace:write",
            "mcp:insight.raise:call",
            "mcp:insight.get:call",
            "mcp:insight.list:call",
            "mcp:tags.add:call",
            "bus:chan/*:pub",
            "bus:chan/*:sub",
        ],
    );

    // The sidecar, approved for the sqlite `127.0.0.1:0` file-source convention.
    install_native(
        &node,
        &OsLauncher,
        &admin,
        ws,
        FEDERATION_MANIFEST,
        &dir,
        &[
            "net:tls:127.0.0.1:0:connect".to_string(),
            "secret:federation/*:get".to_string(),
        ],
        1,
    )
    .await
    .expect("federation sidecar installs + spawns");

    // --- THE ORACLE: one call, and the node becomes an FDD product ---
    let out = apply(&node, &admin, ws, bas_bundle(), 1000)
        .await
        .expect("one pack.apply of the bas fixture");
    assert_eq!(
        out["outcome"], "applied",
        "every object of the bas pack applies: {out}"
    );
    assert_eq!(
        out["ran_rules"], true,
        "the first apply runs the rules: {out}"
    );

    // A REAL insight, raised by a REAL rule reading the REAL seeded datasource.
    let page = insight_list(
        &node.store,
        &admin,
        ws,
        ListQuery {
            filter: Default::default(),
            cursor: None,
            limit: 1000,
        },
    )
    .await
    .expect("insight.list");

    assert!(
        page.items
            .iter()
            .any(|i| i.dedup_key.starts_with("fdd:sensor-flatline:")),
        "the flatline rule must raise against the seeded data — dedup keys seen: {:?}",
        page.items.iter().map(|i| &i.dedup_key).collect::<Vec<_>>()
    );

    let _ = std::fs::remove_dir_all(&lb_dir);
}
