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

// ----- 2b. the entity→table binding: receipt carry + validate lint (Phase B) ---------------------

/// A pack whose entities carry the `{table, pk, parent_fk, display}` binding
/// (`pack-entity-binding-scope.md`). The binding rides `manifest.entities` in the receipt, so
/// `pack.get` returns it verbatim — no new verb, no envelope change. This is the read a downstream
/// surface (the rubix-ai Sites page / entity var) consumes to address a pack's rows as data. A
/// channel object makes the pack applyable without a datasource (so this test needs no sidecar).
fn bound_bundle() -> Value {
    let manifest = "pack: bound\ntitle: Bound Pack\nversion: 1\n\
        entities:\n\
        \x20 site:  { label: Site,  table: site,  pk: id, display: name }\n\
        \x20 meter: { label: Meter, parent: site, table: meter, pk: id, parent_fk: site_id, display: name }\n\
        channels:\n  - name: c\n";
    json!({"manifest": manifest, "files": {}})
}

/// The binding is carried through apply → receipt → `pack.get` unchanged, and an UNBOUND entity keeps
/// exactly today's shape (no null-spammed binding fields — `skip_serializing_if` keeps the promise).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_entity_table_binding_rides_the_receipt_to_pack_get() {
    let ws = "pack-bind";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal(
        ws,
        &[
            "mcp:pack.validate:call",
            "mcp:pack.apply:call",
            "mcp:pack.get:call",
            "bus:chan/*:pub",
            "bus:chan/*:sub",
        ],
    );

    // Validate first: a clean binding lints clean (no gate).
    let v = call(
        &node,
        &admin,
        ws,
        "pack.validate",
        json!({"bundle": bound_bundle()}),
    )
    .await
    .expect("validate bound pack");
    assert_eq!(v["valid"], true, "a well-formed binding lints clean: {v}");

    apply(&node, &admin, ws, bound_bundle(), 1)
        .await
        .expect("apply the bound pack");

    let got = call(&node, &admin, ws, "pack.get", json!({"pack": "bound"}))
        .await
        .expect("pack.get");
    let ents = &got["manifest"]["entities"];
    assert_eq!(ents["site"]["table"], "site", "site binds its table: {got}");
    assert_eq!(ents["site"]["pk"], "id");
    assert_eq!(ents["site"]["display"], "name");
    assert_eq!(
        ents["meter"]["parent_fk"], "site_id",
        "meter carries the parent FK: {got}"
    );
    // parent_fk is absent (not null) on a root entity — the un-broken-promise shape.
    assert!(
        ents["site"].get("parent_fk").is_none(),
        "an unbound field must be ABSENT, not null-spammed: {got}"
    );
}

/// A malformed binding WARNS but does not gate (the dialect-lint precedent) — except `parent_fk` with
/// no `parent`, a manifest-only inconsistency that gates like a dangling parent. An entity with no
/// `table` is byte-for-byte today's shape (the promise is unbroken).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_malformed_binding_warns_but_only_parent_fk_without_parent_gates() {
    let ws = "pack-bind-lint";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    // parent_fk with no parent → an ERROR that gates.
    let bad = json!({"manifest":
        "pack: b\ntitle: B\nversion: 1\n\
         entities:\n  x: { label: X, table: t, pk: id, parent_fk: y_id }\n\
         channels:\n  - name: c\n", "files": {}});
    let out = call(&node, &p, ws, "pack.validate", json!({"bundle": bad}))
        .await
        .expect("validate");
    assert_eq!(out["valid"], false, "parent_fk without parent gates: {out}");

    // A bound table the (opaque, no-schema) pack does not declare → a WARNING, still valid.
    let warn = json!({"manifest":
        "pack: b\ntitle: B\nversion: 1\n\
         entities:\n  x: { label: X, table: ghost, pk: id }\n\
         channels:\n  - name: c\n", "files": {}});
    let out = call(&node, &p, ws, "pack.validate", json!({"bundle": warn}))
        .await
        .expect("validate");
    assert_eq!(
        out["valid"], true,
        "an unverifiable table warns, never gates: {out}"
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

    // ROW 4 — a higher version is an UPGRADE (pack-upgrade-scope): re-drive every object, do NOT
    // re-run rules, and report the version bump loudly. (This small pack has no datasource, so the
    // schema-reconcile step is a no-op — the upgrade of the materialized schema is covered by the
    // sqlite unit tests + the O-1 integration path.)
    let up = apply(&node, &p, ws, small_bundle(2, SMALL_CONTEXT), 13)
        .await
        .expect("a higher version upgrades");
    assert_eq!(up["outcome"], "applied", "the upgrade applied: {up}");
    assert_eq!(up["upgraded"]["from"], 1, "loud version bump: {up}");
    assert_eq!(up["upgraded"]["to"], 2);
    assert_eq!(
        up["ran_rules"], false,
        "an upgrade never re-runs rules: {up}"
    );

    // ROW 5 — a downgrade is always refused. State is now at v2 (the upgrade), so re-applying v1 is a
    // downgrade, not a no-op.
    let err = apply(&node, &p, ws, small_bundle(1, SMALL_CONTEXT), 14)
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

// ----- 8b. the sidebar seed (Kind::Sidebar) -------------------------------------------------------------
//
// The first workspace-seed kind. A `sidebar:` block applies the workspace hidden-set through the
// SAME `nav.hidden.set` a hand-editing admin calls — so the arm proves (a) the set lands verbatim,
// (b) a second apply of the same set is the idempotent no-op, (c) a changed set clobbers loudly, and
// (d) a caller who lacks `nav.save` is DENIED at the arm, never silently skipped. No mocks: the set
// is read back through the real `nav_hidden_get`.

use lb_host::{nav_hidden_get, NavHidden};

/// A pack whose only object is a sidebar hidden-set. `version` and the ref set vary so the tests can
/// drive idempotence (same set, same version → no-op).
fn sidebar_bundle(version: u32, hidden: &[&str]) -> Value {
    let refs = hidden
        .iter()
        .map(|r| format!("    - {r}\n"))
        .collect::<String>();
    let manifest =
        format!("pack: seed\ntitle: Seed Pack\nversion: {version}\nsidebar:\n  hidden:\n{refs}");
    json!({"manifest": manifest, "files": {}})
}

/// A sidebar block PLUS a channel — two objects, so the clobber test can drive the real re-apply
/// path: the refusal matrix only re-applies (and clobbers) at the SAME version when the prior receipt
/// was PARTIAL (a same-version+changed-content bump is refused, "bump the version"; a higher version
/// is refused, "upgrade not built"). So a first apply with the channel cap withheld leaves sidebar
/// applied + channel denied — a partial — and the recovery re-apply clobbers the already-applied
/// sidebar, listing it loudly. That is the shipped clobber contract, exercised honestly.
fn sidebar_and_channel_bundle(hidden: &[&str]) -> Value {
    let refs = hidden
        .iter()
        .map(|r| format!("    - {r}\n"))
        .collect::<String>();
    let manifest = format!(
        "pack: seed\ntitle: Seed Pack\nversion: 1\n\
         channels:\n  - name: seed-alerts\n\
         sidebar:\n  hidden:\n{refs}"
    );
    json!({"manifest": manifest, "files": {}})
}

/// The pack surface + `nav.save` (the write the arm rides) + `nav.resolve` (so the test can read the
/// set back). This is the full grant a sidebar apply needs — and nothing downstream of it.
fn sidebar_full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&["mcp:nav.save:call", "mcp:nav.resolve:call"]);
    principal(ws, &caps)
}

/// The pack surface + `nav.resolve` (to read back) but NOT `nav.save`. A caller who cannot shape the
/// workspace's menus by hand must not be able to hide a surface via a pack either — the arm denies.
fn sidebar_missing_nav_save(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.push("mcp:nav.resolve:call");
    principal(ws, &caps)
}

/// Read the workspace hidden-set back through the real read verb, as a plain `Vec<String>`.
async fn read_hidden(node: &Arc<Node>, p: &Principal, ws: &str) -> Vec<String> {
    let NavHidden { hidden, .. } = nav_hidden_get(&node.store, p, ws)
        .await
        .expect("nav_hidden_get");
    hidden
}

/// The headline: a `sidebar:` block on a blank workspace lands the hidden-set EXACTLY as declared,
/// through `nav.hidden.set` — nothing in the arm interprets a ref.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_sidebar_block_applies_the_hidden_set_exactly() {
    let ws = "seed-sidebar";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = sidebar_full(ws);

    // Blank workspace → nothing hidden.
    assert!(
        read_hidden(&node, &p, ws).await.is_empty(),
        "a blank workspace hides nothing to start"
    );

    let resp = apply(
        &node,
        &p,
        ws,
        sidebar_bundle(1, &["channels", "datasources"]),
        10,
    )
    .await
    .expect("apply");
    assert_eq!(resp["outcome"], "applied", "{resp}");
    assert_eq!(
        object_outcome(&resp, "sidebar"),
        "applied",
        "the sidebar object applied: {resp}"
    );

    assert_eq!(
        read_hidden(&node, &p, ws).await,
        vec!["channels", "datasources"],
        "the hidden-set landed exactly as declared"
    );
}

/// A second apply of the SAME set at the same version is the idempotent no-op — the refusal matrix
/// short-circuits before the arm runs, and the set is unchanged.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_applying_the_same_sidebar_set_is_a_no_op() {
    let ws = "seed-sidebar-noop";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = sidebar_full(ws);

    apply(&node, &p, ws, sidebar_bundle(1, &["channels"]), 10)
        .await
        .expect("first apply");
    let again = apply(&node, &p, ws, sidebar_bundle(1, &["channels"]), 11)
        .await
        .expect("second apply");
    assert_eq!(again["outcome"], "noop", "same set, same version: {again}");
    assert_eq!(read_hidden(&node, &p, ws).await, vec!["channels"]);
}

/// A re-apply CLOBBERS the pack-owned sidebar object loudly (`sidebar:seed` in the run's clobber
/// list), and the full-set LWW re-writes the hidden-set. This drives the ONLY same-version re-apply
/// the matrix allows: a partial-recovery. The first apply withholds the channel cap, so sidebar
/// applies but the channel is denied — a partial; the recovery re-apply (channel cap granted) clobbers
/// the already-applied sidebar. Same contract as the agent/dashboard clobber, exercised honestly.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reapply_clobbers_the_sidebar_object_loudly() {
    let ws = "seed-sidebar-clobber";
    let node = Arc::new(Node::boot().await.unwrap());

    // sidebar caps but NOT the channel `pub`/`sub` gate → sidebar applies, channel denied → partial.
    let partial_p = {
        let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
        caps.extend_from_slice(&["mcp:nav.save:call", "mcp:nav.resolve:call"]);
        principal(ws, &caps)
    };
    // Everything the pack needs → the recovery re-apply completes, clobbering the applied sidebar.
    let full_p = {
        let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
        caps.extend_from_slice(&[
            "mcp:nav.save:call",
            "mcp:nav.resolve:call",
            "bus:chan/*:pub",
            "bus:chan/*:sub",
        ]);
        principal(ws, &caps)
    };

    let first = apply(
        &node,
        &partial_p,
        ws,
        sidebar_and_channel_bundle(&["channels"]),
        10,
    )
    .await
    .expect("first apply");
    assert_eq!(
        first["outcome"], "partial",
        "channel denied → partial: {first}"
    );
    assert_eq!(object_outcome(&first, "sidebar"), "applied", "{first}");
    assert_eq!(object_outcome(&first, "channel"), "denied", "{first}");
    assert_eq!(
        first["clobbered"].as_array().unwrap().len(),
        0,
        "a first apply overwrites nothing: {first}"
    );

    // The recovery re-apply (same version + same content, prior partial) re-runs the plan and
    // clobbers the objects that were already applied — the sidebar among them, named loudly.
    let again = apply(
        &node,
        &full_p,
        ws,
        sidebar_and_channel_bundle(&["channels"]),
        11,
    )
    .await
    .expect("recovery re-apply");
    assert_eq!(again["outcome"], "applied", "recovery completes: {again}");
    let clobbered: Vec<&str> = again["clobbered"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(
        clobbered.contains(&"sidebar:seed"),
        "the sidebar overwrite is listed loudly: {again}"
    );
    assert_eq!(
        read_hidden(&node, &full_p, ws).await,
        vec!["channels"],
        "full-set LWW re-wrote the hidden-set"
    );
}

/// The caps wall: a caller WITHOUT `nav.save` is denied at the arm — the sidebar object is `denied`
/// in the receipt (a partial), never silently skipped, and the workspace stays un-hidden. Proves a
/// pack grants no privileged path past the same gate the hand path hits.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_caller_without_nav_save_is_denied_the_sidebar_arm() {
    let ws = "seed-sidebar-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let short = sidebar_missing_nav_save(ws);

    let resp = apply(&node, &short, ws, sidebar_bundle(1, &["channels"]), 10)
        .await
        .expect("apply returns (a partial, not an error)");
    assert_eq!(
        resp["outcome"], "partial",
        "one grant short is a partial, not a success: {resp}"
    );
    assert_eq!(
        object_outcome(&resp, "sidebar"),
        "denied",
        "the sidebar object is denied at the arm: {resp}"
    );
    // And the workspace is untouched — a denied hide leaves the rail as it was.
    assert!(
        read_hidden(&node, &short, ws).await.is_empty(),
        "a denied apply hid nothing: the wall held"
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

// ----- 11. O-1: federation.write reaches a pack's in-process sqlite materialized source ----------

/// **O-1 — the entity-data-plane pivot** (rubix-ai `entity-data-plane-scope.md`; lb
/// `pack-entity-binding-scope.md` §Risks/O-1). `federation.write` is documented for a *registered
/// external source* and gates on the source's `net:*` endpoint; a pack's datasource is an **in-process
/// sqlite materializer** with a node-local file DSN. This test settles, live, whether `federation.write`
/// reaches it — the fact the whole downstream data plane turns on. It applies the real `bas` pack (which
/// materializes `demo-buildings`), then:
///   1. writes a NEW `site` row and reads it back (INSERT reaches the file);
///   2. UPSERTs the same PK twice and asserts it lands once (idempotence);
///   3. edits a SEEDED site's coordinate via UPSERT (operator edit of pack data);
///   4. RE-APPLIES `bas` and asserts the operator's new row + edit SURVIVE — the seed-ownership rule
///      (`pack-entity-binding-scope.md` §"seed-ownership decision"): seeded rows are starting data
///      applied once, never re-clobbered.
///
/// `#[ignore]` for the same reason as the demo oracle: it needs the real `federation` sidecar built.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs the real federation sidecar built: cargo build -p federation"]
async fn o1_federation_write_reaches_the_pack_materialized_sqlite_source() {
    use lb_host::install_native;
    use lb_supervisor::OsLauncher;

    const FEDERATION_MANIFEST: &str = include_str!("../../federation/extension.toml");

    let lb_dir = std::env::temp_dir().join(format!("lb-o1-write-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lb_dir);
    std::env::set_var("LB_DIR", &lb_dir);

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

    let ws = "o1";
    let node = Arc::new(Node::boot().await.unwrap());
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
            "mcp:federation.write:call",
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
            "mcp:tags.add:call",
            "bus:chan/*:pub",
            "bus:chan/*:sub",
        ],
    );

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

    apply(&node, &admin, ws, bas_bundle(), 1000)
        .await
        .expect("one pack.apply of the bas fixture");

    // How many sites did the seed plant? (Baseline — the operator's add must be seen ON TOP.)
    let count_sites = |node: &Arc<Node>, admin: &Principal| {
        let node = node.clone();
        let admin = admin.clone();
        async move {
            call(
                &node,
                &admin,
                ws,
                "federation.query",
                json!({"source":"demo-buildings","sql":"SELECT id, name FROM site ORDER BY id","ts":2}),
            )
            .await
            .expect("federation.query sites")
        }
    };
    let seeded = count_sites(&node, &admin).await;
    let seeded_n = seeded["rows"].as_array().expect("rows").len();
    assert!(seeded_n > 0, "the bas seed plants sites: {seeded}");

    // --- 1. INSERT a NEW site via federation.write, and read it back ---------------------------
    let w = call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo-buildings",
            "table": "site",
            "columns": ["id", "name"],
            "rows": [["site-o1", "Riverside Annex 2"]],
            "key": ["id"],
        }),
    )
    .await
    .expect("federation.write reaches the in-process sqlite materialized source (O-1 = YES)");
    assert_eq!(w["affected"], 1, "one row written: {w}");

    let after = count_sites(&node, &admin).await;
    assert_eq!(
        after["rows"].as_array().unwrap().len(),
        seeded_n + 1,
        "the written row is now readable back from the materialized source: {after}"
    );
    assert!(
        after.to_string().contains("Riverside Annex 2"),
        "the new site's name round-trips: {after}"
    );

    // --- 2. UPSERT idempotence: writing the same PK twice lands ONE row -------------------------
    call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo-buildings", "table": "site",
            "columns": ["id", "name"],
            "rows": [["site-o1", "Riverside Annex 2 (v2)"]],
            "key": ["id"],
        }),
    )
    .await
    .expect("redelivered UPSERT");
    let deduped = count_sites(&node, &admin).await;
    assert_eq!(
        deduped["rows"].as_array().unwrap().len(),
        seeded_n + 1,
        "the UPSERT updated in place — no duplicate PK: {deduped}"
    );

    // --- 3. Edit a SEEDED site in place (operator editing pack-seeded data) ---------------------
    let first_seeded_id = seeded["rows"][0][0]
        .as_str()
        .expect("seeded id")
        .to_string();
    call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({
            "source": "demo-buildings", "table": "site",
            "columns": ["id", "name"],
            "rows": [[first_seeded_id, "OPERATOR-EDITED"]],
            "key": ["id"],
        }),
    )
    .await
    .expect("edit a seeded row via UPSERT");

    // --- 4. RE-APPLY the pack: the operator's add + edit MUST survive (seed-ownership rule) ------
    apply(&node, &admin, ws, bas_bundle(), 2000)
        .await
        .expect("re-apply of the bas fixture");

    let survived = count_sites(&node, &admin).await;
    let txt = survived.to_string();
    assert!(
        txt.contains("Riverside Annex 2 (v2)"),
        "SEED OWNERSHIP: the operator-added site must survive a pack re-apply (it did NOT — \
         materialize() rebuilds the db fresh; the seed-ownership rule is the required lb fix): {survived}"
    );
    assert!(
        txt.contains("OPERATOR-EDITED"),
        "SEED OWNERSHIP: the operator's edit to a seeded row must survive re-apply: {survived}"
    );

    let _ = std::fs::remove_dir_all(&lb_dir);
}

// ----- 12. pack UPGRADE end-to-end (a version bump reconciles the schema, preserves rows) ----------

/// **The pack-upgrade oracle** (`pack-upgrade-scope.md`): a version bump migrates the materialized
/// schema ADDITIVELY while PRESERVING the operator's rows — the thing that lets a pack evolve without
/// abandoning a workspace's data. Applies a minimal sqlite pack at v1 (`site(id,name)`, seeded), writes
/// an operator row, then applies v2 whose schema adds a `lat` column. Asserts: the response reports the
/// upgrade (`from:1,to:2`), the operator's row SURVIVED, the new column EXISTS (and is null on the old
/// row), and the receipt is now v2. `#[ignore]` for the same reason as the O-1 test — the real sidecar.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs the real federation sidecar built: cargo build -p federation"]
async fn pack_upgrade_reconciles_schema_and_preserves_rows() {
    use lb_host::install_native;
    use lb_supervisor::OsLauncher;

    const FEDERATION_MANIFEST: &str = include_str!("../../federation/extension.toml");

    let lb_dir = std::env::temp_dir().join(format!("lb-upgrade-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lb_dir);
    std::env::set_var("LB_DIR", &lb_dir);

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
                "federation sidecar builds"
            );
            target.to_string_lossy().into_owned()
        }
    };

    let ws = "upgrade";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal(
        ws,
        &[
            "mcp:pack.validate:call",
            "mcp:pack.apply:call",
            "mcp:pack.get:call",
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:native.status:call",
            "mcp:datasource.add:call",
            "mcp:datasource.list:call",
            "mcp:datasource.test:call",
            "mcp:federation.query:call",
            "mcp:federation.write:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    );

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

    // A minimal pack: one sqlite datasource `demo`, `site(id,name)`, seeded with one row.
    let bundle_v1 = json!({
        "manifest": "pack: up\ntitle: Up\nversion: 1\n\
            datasource:\n  name: demo\n  engine: sqlite\n  schema: schema.sql\n  seed: seed.sql\n",
        "files": {
            "schema.sql": "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL);",
            "seed.sql": "INSERT INTO site VALUES ('seed-1','Seeded Site');",
        },
    });
    apply(&node, &admin, ws, bundle_v1, 1000)
        .await
        .expect("apply v1");

    // The operator adds a row (their data — must survive the upgrade).
    call(
        &node,
        &admin,
        ws,
        "federation.write",
        json!({"source":"demo","table":"site","columns":["id","name"],
               "rows":[["op-1","Operator Site"]],"key":["id"]}),
    )
    .await
    .expect("operator writes a row");

    // v2: the SAME pack, version bumped, schema adds a nullable `lat` column.
    let bundle_v2 = json!({
        "manifest": "pack: up\ntitle: Up\nversion: 2\n\
            datasource:\n  name: demo\n  engine: sqlite\n  schema: schema.sql\n  seed: seed.sql\n",
        "files": {
            "schema.sql": "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL, lat REAL);",
            "seed.sql": "INSERT INTO site VALUES ('seed-1','Seeded Site');",
        },
    });
    let up = apply(&node, &admin, ws, bundle_v2, 2000)
        .await
        .expect("apply v2 = upgrade");
    assert_eq!(up["outcome"], "applied", "the upgrade applied: {up}");
    assert_eq!(up["upgraded"]["from"], 1, "loud version bump: {up}");
    assert_eq!(up["upgraded"]["to"], 2);

    // The reconcile is host-side (out-of-band to the sidecar's warm connection), so re-check the
    // source to drop any pool connection opened before the migration — `datasource.test` is the
    // "this source changed shape, drop what you're holding" lever (query.rs::probe evicts on it).
    call(
        &node,
        &admin,
        ws,
        "datasource.test",
        json!({"source":"demo","ts":25}),
    )
    .await
    .expect("probe re-checks the migrated source");

    // The new column EXISTS now.
    let cols = call(
        &node,
        &admin,
        ws,
        "federation.schema",
        json!({"source":"demo","table":"site","ts":3}),
    )
    .await
    .expect("schema after upgrade");
    let names: Vec<&str> = cols["columns"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"lat"),
        "the added column exists after upgrade: {cols}"
    );

    // BOTH rows SURVIVED (the seed row + the operator's), and `lat` is null on them.
    let rows = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({"source":"demo","sql":"SELECT id, lat FROM site ORDER BY id","ts":4}),
    )
    .await
    .expect("query after upgrade");
    let arr = rows["rows"].as_array().unwrap();
    assert_eq!(arr.len(), 2, "both rows survived the upgrade: {rows}");
    assert!(
        rows.to_string().contains("op-1"),
        "the operator's row survived: {rows}"
    );

    // The receipt is now v2.
    let got = call(&node, &admin, ws, "pack.get", json!({"pack":"up"}))
        .await
        .expect("pack.get");
    assert_eq!(
        got["version"], 2,
        "the receipt records the new version: {got}"
    );

    let _ = std::fs::remove_dir_all(&lb_dir);
}

// ----- retention (pack-retention-scope) ----------------------------------------------------------
//
// A `retention:` block is a new closed-`Kind` object (Kind::Retention): inline policies applied via
// `series.retention.set` under the caller's principal. These prove the four contracts of the family
// for this arm — applies + is readable, the per-object cap wall fires, idempotent+drift, workspace
// isolation — against the real node (no mocks), driving the same `series_retention_set` the verb does.

/// A bundle with a `retention:` block (+ a channel so the pack has a second object). Inline policies,
/// no bundle files. `raw`/`title` vary so a test can force drift.
fn retention_bundle(raw_for_ms: u64, title: &str) -> Value {
    json!({"manifest": format!(
        "pack: ret\ntitle: {title}\nversion: 1\n\
         channels:\n  - name: c\n\
         retention:\n  - prefix: \"modbus.\"\n    raw_for_ms: {raw_for_ms}\n    max_samples: 5000\n    tiers:\n      - {{width_ms: 60000, keep_for_ms: 604800000}}\n"
    ), "files": {}})
}

/// The pack surface + the two per-object caps this bundle re-checks (the channel `pub`/`sub` gate and
/// the retention setter). Deliberately granular so a test can drop just the retention cap.
fn retention_full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "bus:chan/*:pub",
        "bus:chan/*:sub",
        "mcp:series.retention.set:call",
        "mcp:series.retention.list:call",
    ]);
    principal(ws, &caps)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_block_applies_and_is_readable() {
    let ws = "ws-ret-apply";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = retention_full(ws);

    let resp = apply(&node, &p, ws, retention_bundle(3_600_000, "T"), 1)
        .await
        .expect("apply");
    assert_eq!(object_outcome(&resp, "retention"), "applied", "{resp}");

    // The policy is now set — `series.retention.list` returns it exactly as declared.
    let list = call(&node, &p, ws, "series.retention.list", json!({}))
        .await
        .expect("list");
    let pol = list["policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["prefix"] == "modbus.")
        .unwrap_or_else(|| panic!("modbus. policy present: {list}"));
    assert_eq!(pol["raw_for_ms"], 3_600_000);
    assert_eq!(pol["max_samples"], 5_000);
    assert_eq!(pol["tiers"][0]["width_ms"], 60_000);
    assert_eq!(pol["tiers"][0]["keep_for_ms"], 604_800_000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_object_is_denied_without_the_setter_cap() {
    let ws = "ws-ret-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // The pack surface + the channel gate, but NOT the retention setter → a PARTIAL: the channel
    // applies, the retention object is denied and listed, no policy is written.
    let missing = principal(ws, &{
        let mut c: Vec<&str> = PACK_SURFACE.to_vec();
        c.extend_from_slice(&["bus:chan/*:pub", "bus:chan/*:sub", "mcp:series.retention.list:call"]);
        c
    });

    let resp = apply(&node, &missing, ws, retention_bundle(3_600_000, "T"), 1)
        .await
        .expect("apply");
    assert_eq!(object_outcome(&resp, "channel"), "applied", "{resp}");
    assert_eq!(object_outcome(&resp, "retention"), "denied", "{resp}");

    // No policy was written — the deny is real, not cosmetic.
    let list = call(&node, &missing, ws, "series.retention.list", json!({}))
        .await
        .expect("list");
    assert!(
        list["policies"].as_array().unwrap().is_empty(),
        "no policy set on a denied apply: {list}"
    );

    // Grant the cap + re-apply recovers it (the pack-core partial→grant→recover matrix, this arm).
    let full = retention_full(ws);
    let resp2 = apply(&node, &full, ws, retention_bundle(3_600_000, "T"), 2)
        .await
        .expect("re-apply");
    assert_eq!(object_outcome(&resp2, "retention"), "applied", "{resp2}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_reapply_is_noop_and_a_changed_policy_drifts() {
    let ws = "ws-ret-drift";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = retention_full(ws);

    apply(&node, &p, ws, retention_bundle(3_600_000, "T"), 1)
        .await
        .expect("apply 1");

    // SAME bundle → NoOp (unchanged checksum): the response carries no re-applied objects.
    let noop = apply(&node, &p, ws, retention_bundle(3_600_000, "T"), 2)
        .await
        .expect("re-apply same");
    assert_eq!(noop["outcome"], "noop", "unchanged pack is a NoOp: {noop}");

    // A changed policy at a BUMPED version re-applies and `list` reflects the new horizon.
    let changed = json!({"manifest":
        "pack: ret\ntitle: T\nversion: 2\n\
         channels:\n  - name: c\n\
         retention:\n  - prefix: \"modbus.\"\n    raw_for_ms: 7200000\n    max_samples: 5000\n    tiers:\n      - {width_ms: 60000, keep_for_ms: 604800000}\n", "files": {}});
    apply(&node, &p, ws, changed, 3).await.expect("apply changed");
    let list = call(&node, &p, ws, "series.retention.list", json!({}))
        .await
        .expect("list");
    let pol = list["policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|x| x["prefix"] == "modbus.")
        .unwrap();
    assert_eq!(pol["raw_for_ms"], 7_200_000, "the bumped horizon applied: {list}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn retention_policies_do_not_cross_the_workspace_wall() {
    let node = Arc::new(Node::boot().await.unwrap());
    let pa = retention_full("ws-ret-a");
    let pb = retention_full("ws-ret-b");

    apply(&node, &pa, "ws-ret-a", retention_bundle(3_600_000, "A"), 1)
        .await
        .expect("apply A");

    // ws-B has NOT applied anything → its retention list is empty (no read of ws-A's policy).
    let list_b = call(&node, &pb, "ws-ret-b", "series.retention.list", json!({}))
        .await
        .expect("list B");
    assert!(
        list_b["policies"].as_array().unwrap().is_empty(),
        "ws-B sees no policy from ws-A: {list_b}"
    );
}
