//! `proof-panel` end to end (proof-panel scope): the reference **Tier-1 WASM** self-contained
//! extension — a real MCP tool served from the wasm guest + a federated page reaching real series
//! through the host-mediated bridge, in ONE folder. Proven through the REAL publish→install→load→call
//! path with the REAL `proof_panel_ext.wasm` component (no mocks — CLAUDE §9): real embedded
//! SurrealDB, real `lb-runtime` component runtime, real capability gate.
//!
//! What each test proves:
//!   1. `proof_ping_is_callable_after_publish` — a signed artifact publishes-installs-loads and the
//!      Tier-1 tool `proof-panel.proof.ping` is callable RIGHT NOW; the snapshot round-trips with
//!      `tier == "wasm"`, proving the wasm component (not a native child) served the call.
//!   2. `proof_ping_is_denied_without_the_grant` — MANDATORY capability-deny: refused, opaquely,
//!      without `mcp:proof-panel.proof.ping:call` (the grant lives on the CALLER, the hello
//!      convention — the manifest requests no host-side cap for its own tool).
//!   3. `grant_intersection_denies_the_unapproved_verb_at_the_bridge` — install with an approval that
//!      OMITS `series.latest`; the persisted page scope drops it (requested ∩ admin_approved), AND a
//!      bridge `series.latest` call by a principal carrying only the granted set is denied at CALL
//!      time, surfaced as an honest error — not merely hidden in the UI.
//!   4. `workspace_isolation_series_and_ping` — two real workspaces seeded independently; ws-B's
//!      bridge `series.find` (with the grant) sees NONE of ws-A's series, and ws-B's `proof.ping`
//!      (lacking the grant) is denied. The hard wall, through the real host bridge.
//!
//! The bridge entry under test is `lb_host::call_tool` — the SAME function the gateway's `POST
//! /mcp/call` route forwards a page's `{tool, args}` through. (Before this slice, `call_tool` could
//! not dispatch a host-native `series.*` verb at all — it resolved only the runtime registry; see
//! debugging/extensions/bridge-cannot-dispatch-host-native-series.md.)

use ed25519_dalek::{Signer, SigningKey as PublisherSigningKey};
use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    call_tool, drain_workspace, ext_list, ext_publish, ingest_write, install_extension, installed,
    load_extension, Node, Qos, Sample,
};
use lb_inbox::{record, Item};
use lb_mcp::ToolError;
use lb_outbox::{enqueue, Effect};
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_tags::{Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};
use std::sync::Arc;

const MANIFEST: &str = include_str!("../../../extensions/proof-panel/extension.toml");

const PING: &str = "mcp:proof-panel.proof.ping:call";
const PUBLISH: &str = "mcp:ext.publish:call";
const FIND: &str = "mcp:series.find:call";
const LATEST: &str = "mcp:series.latest:call";
const WRITE: &str = "mcp:ingest.write:call";
const OUTBOX: &str = "mcp:outbox.status:call";
const INBOX_LIST: &str = "mcp:inbox.list:call";
const INBOX_RESOLVE: &str = "mcp:inbox.resolve:call";
const INBOX_RECORD: &str = "mcp:inbox.record:call";
const OUTBOX_ENQUEUE: &str = "mcp:outbox.enqueue:call";
const SIMULATE: &str = "mcp:proof-panel.proof.simulate:call";

/// The built Tier-1 component. Panics with the build hint if missing (the real wasm, not a mock).
fn proof_panel_wasm() -> Vec<u8> {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions/proof-panel/target/wasm32-wasip2/release/proof_panel_ext.wasm");
    std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "missing component at {} ({e}).\nBuild it first:\n  bash rust/extensions/proof-panel/build.sh",
            path.display()
        )
    })
}

fn principal(ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:test".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    verify(&key, &mint(&key, &claims), 1).expect("token verifies")
}

/// A deterministic dev publisher (fixed seed) + the matching trust allow-list.
fn publisher(seed: u8) -> (String, PublisherSigningKey, TrustedKeys) {
    let sk = PublisherSigningKey::from_bytes(&[seed; 32]);
    let id = format!("pub-{seed}");
    let pk = PublisherKey::from_bytes(&sk.verifying_key().to_bytes()).unwrap();
    (id.clone(), sk, TrustedKeys::from([(id, pk)]))
}

fn sign(wasm: &[u8], key_id: &str, sk: &PublisherSigningKey) -> Artifact {
    let d = digest(MANIFEST, wasm);
    Artifact {
        ext_id: "proof-panel".into(),
        version: "0.1.0".into(),
        manifest_toml: MANIFEST.into(),
        wasm: wasm.to_vec(),
        digest_hex: digest_hex(&d),
        publisher_key_id: key_id.into(),
        signature: sk.sign(&d).to_bytes().to_vec(),
    }
}

/// Publish-install-load a signed `proof-panel` into `node`/`ws`. Returns once the Install persists.
async fn publish_into(node: &Node, ws: &str, seed: u8) {
    let (kid, sk, trusted) = publisher(seed);
    let art = sign(&proof_panel_wasm(), &kid, &sk);
    let caller = principal(ws, &[PUBLISH]);
    ext_publish(node, &caller, ws, art, &trusted, Visibility::Private, 1)
        .await
        .expect("a signed proof-panel artifact publishes-and-installs-and-loads");
    let rec = installed(node, ws, "proof-panel")
        .await
        .unwrap()
        .expect("installed record");
    assert_eq!(rec.version, "0.1.0");
}

/// Seed a real, discoverable series — through the REAL write paths, never a mocked row:
///   1. stage + drain one sample so the committed `series` row exists (so `series.latest` reads it);
///   2. apply a real `kind:temperature` tag edge on the `series:<name>` entity through `lb_tags::add`,
///      so `series.find` (which intersects the tag graph) can discover it.
///
/// Step 2 is explicit because the ingest write+drain path does NOT convert a sample's `labels` into
/// tag edges today — the `Sample.labels` "converted to tag edges at commit" behaviour is unimplemented
/// (see debugging/extensions/series-find-needs-tag-edges-not-labels.md). The page's discovery is via
/// `series.find`, so the test creates the tag edge a producer's labels *should* eventually produce.
async fn seed_series(node: &Node, ws: &str, series: &str, seq: u64, payload: f64) {
    let p = principal(ws, &["mcp:ingest.write:call"]);
    let sample = Sample {
        series: series.into(),
        producer: String::new(),
        ts: seq,
        seq,
        payload: serde_json::json!(payload),
        labels: serde_json::json!({ "kind": "temperature" }),
        qos: Qos::BestEffort,
    };
    ingest_write(&node.store, &p, ws, vec![sample])
        .await
        .expect("stage sample");
    drain_workspace(&node.store, ws)
        .await
        .expect("commit drain");

    // The discoverability edge: tag the `series:<name>` entity with `kind:temperature` (the real tag
    // write path). `series.find` keeps only `series:`-prefixed entities, so this is exactly what makes
    // the seeded series appear in a faceted query.
    let tag = Tag::new("kind", serde_json::json!("temperature"));
    let prov = Provenance::new(seq, "user:test", Source::Producer);
    lb_tags::add(
        &node.store,
        ws,
        &format!("series:{series}"),
        &tag,
        &prov,
        DEFAULT_TAG_NODE_CAP,
    )
    .await
    .expect("tag the series entity for discovery");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_ping_is_callable_after_publish() {
    let ws = "proof-happy";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    publish_into(&node, ws, 40).await;

    // The Tier-1 tool is callable RIGHT NOW — publish loaded the wasm component in-process. Routed
    // through `call_tool` (the gateway's bridge entry), exactly as a page's call would be.
    let caller = principal(ws, &[PING]);
    let out = call_tool(
        &node,
        &caller,
        ws,
        "proof-panel.proof.ping",
        &format!(r#"{{"ws":"{ws}"}}"#),
    )
    .await
    .expect("proof.ping on the just-published wasm extension");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["ws"], ws, "the caller's workspace round-trips");
    assert_eq!(v["node"], "proof-panel");
    assert_eq!(v["tier"], "wasm", "the Tier-1 component served the call");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_ping_is_denied_without_the_grant() {
    let ws = "proof-deny";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    publish_into(&node, ws, 41).await;

    // A caller WITHOUT mcp:proof-panel.proof.ping:call is denied — opaquely (no existence signal).
    let ungranted = principal(ws, &[]);
    let err = call_tool(&node, &ungranted, ws, "proof-panel.proof.ping", "{}")
        .await
        .expect_err("proof.ping without the grant is denied");
    assert!(
        matches!(err, ToolError::Denied),
        "denial is opaque, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn grant_intersection_denies_the_unapproved_verb_at_the_bridge() {
    // The manifest requests series.find + series.latest + series.read. The admin approves ONLY
    // series.find. The persisted page scope must drop series.latest (requested ∩ admin_approved), AND
    // a bridge series.latest call carrying only the granted set is denied at CALL time — the narrowing
    // is enforced, not merely displayed.
    let ws = "proof-intersect";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let approved = vec![FIND.to_string()]; // series.latest deliberately omitted
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &approved, 1)
        .await
        .expect("install with a narrowed approval");

    // 1. the persisted page scope dropped series.latest (it was never approved).
    let admin = principal(ws, &["mcp:ext.list:call"]);
    let rows = ext_list(&node, &admin, ws).await.expect("ext.list");
    let page = rows
        .iter()
        .find(|r| r.ext == "proof-panel")
        .expect("proof-panel row")
        .ui
        .as_ref()
        .expect("[ui] page surfaced");
    assert_eq!(
        page.scope,
        vec!["series.find".to_string()],
        "series.latest dropped from the page scope — not approved"
    );

    // 2. and at CALL time: a page principal carrying exactly the granted set has find work but latest
    // denied at the bridge (the host re-checks; this is the load-bearing guarantee, not UI hiding).
    seed_series(&node, ws, "edge.temp", 1, 21.0).await;
    let page_principal = principal(ws, &[FIND]); // granted find, NOT latest
    let found = call_tool(
        &node,
        &page_principal,
        ws,
        "series.find",
        r#"{"facets":[{"key":"kind","value":"temperature"}]}"#,
    )
    .await
    .expect("series.find is granted");
    let fv: serde_json::Value = serde_json::from_str(&found).unwrap();
    assert!(
        fv["series"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s == "series:edge.temp" || s == "edge.temp"),
        "find lists the seeded series, got {fv}"
    );
    let err = call_tool(
        &node,
        &page_principal,
        ws,
        "series.latest",
        r#"{"series":"edge.temp"}"#,
    )
    .await
    .expect_err("series.latest is denied — the narrowing is enforced at the bridge");
    assert!(
        matches!(err, ToolError::Denied),
        "denial is opaque, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workspace_isolation_series_and_ping() {
    // Two real workspaces on the same node. ws-A is seeded with a series; ws-B is not. ws-B's bridge
    // series.find (with the grant) sees NONE of ws-A's series — the hard wall on the read. And ws-B's
    // proof.ping without the grant is denied (a data-less tool's per-ws wall bites at the cap gate).
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    publish_into(&node, "iso-a", 42).await;
    publish_into(&node, "iso-b", 43).await;

    // Seed ws-A only.
    seed_series(&node, "iso-a", "a.secret", 1, 99.0).await;

    // ws-B, with series.find granted, sees an EMPTY result for the same facet (none of ws-A's series).
    let b_reader = principal("iso-b", &[FIND, LATEST]);
    let out = call_tool(
        &node,
        &b_reader,
        "iso-b",
        "series.find",
        r#"{"facets":[{"key":"kind","value":"temperature"}]}"#,
    )
    .await
    .expect("ws-B find is granted");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["series"].as_array().unwrap().len(),
        0,
        "ws-B sees NONE of ws-A's series — the hard wall, got {v}"
    );

    // ws-B's proof.ping WITHOUT the grant is denied (capability wall on the data-less tool).
    let b_ungranted = principal("iso-b", &[]);
    let err = call_tool(&node, &b_ungranted, "iso-b", "proof-panel.proof.ping", "{}")
        .await
        .expect_err("ws-B proof.ping without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

// ── The demo's full round-trip through the bridge: ingest.write → series.latest, plus the durable
// workflow surface (outbox.status, inbox.list, inbox.resolve) — each reached the SAME way the
// proof-panel page reaches it: `call_tool` (the `POST /mcp/call` bridge entry). Real store, real gate.

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ingest_write_then_latest_round_trips_through_the_bridge() {
    // The headline: the page CREATES the data it shows. Write a sample over the bridge → drain (the
    // node's commit worker) → read it back via series.latest over the same bridge. End to end.
    let ws = "demo-write";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let caller = principal(ws, &[WRITE, LATEST]);

    let n = call_tool(
        &node,
        &caller,
        ws,
        "ingest.write",
        r#"{"samples":[{"series":"proof.demo","producer":"","ts":1,"seq":1,"payload":42.5,"labels":null,"qos":"best-effort"}]}"#,
    )
    .await
    .expect("ingest.write is granted");
    let wv: serde_json::Value = serde_json::from_str(&n).unwrap();
    assert_eq!(wv["accepted"], 1, "one sample staged, got {wv}");

    // The commit worker drains staging → the `series` row (the node does this in production).
    drain_workspace(&node.store, ws).await.expect("drain");

    let out = call_tool(
        &node,
        &caller,
        ws,
        "series.latest",
        r#"{"series":"proof.demo"}"#,
    )
    .await
    .expect("series.latest is granted");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["sample"]["payload"], 42.5,
        "the page reads back the value it just wrote, got {v}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ingest_write_is_denied_without_the_grant() {
    let ws = "demo-write-deny";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    let ungranted = principal(ws, &[]); // no mcp:ingest.write:call
    let err = call_tool(
        &node,
        &ungranted,
        ws,
        "ingest.write",
        r#"{"samples":[{"series":"x","producer":"","ts":1,"seq":1,"payload":1,"labels":null,"qos":"best-effort"}]}"#,
    )
    .await
    .expect_err("ingest.write without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn outbox_status_reads_real_effects_and_denies_without_the_grant() {
    let ws = "demo-outbox";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());

    // Seed a real outbox effect through the real enqueue path (the same write start_job performs).
    let effect = Effect::new("e1", "github", "comment", "hi", "e1", 1);
    enqueue(
        &node.store,
        ws,
        "seed_change",
        "e1",
        &serde_json::json!({ "seeded": true }),
        &effect,
    )
    .await
    .expect("enqueue a real effect");

    let granted = principal(ws, &[OUTBOX]);
    let out = call_tool(&node, &granted, ws, "outbox.status", "{}")
        .await
        .expect("outbox.status is granted");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["pending"].as_array().unwrap().len(),
        1,
        "the seeded effect shows as pending, got {v}"
    );

    // MANDATORY deny: a principal without the grant is refused, opaquely.
    let ungranted = principal(ws, &[]);
    let err = call_tool(&node, &ungranted, ws, "outbox.status", "{}")
        .await
        .expect_err("outbox.status without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_list_then_resolve_round_trips_and_denies_per_verb() {
    let ws = "demo-inbox";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());

    // Seed a real durable inbox item through the real record path.
    let item = Item::new("i1", "triage", "ext:demo", "please review", 1);
    record(&node.store, ws, &item)
        .await
        .expect("record a real inbox item");

    // inbox.list (granted) returns it.
    let lister = principal(ws, &[INBOX_LIST]);
    let out = call_tool(&node, &lister, ws, "inbox.list", r#"{"channel":"triage"}"#)
        .await
        .expect("inbox.list is granted");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let items = v["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "the seeded item is listed, got {v}");
    assert_eq!(items[0]["id"], "i1");

    // inbox.list deny (MANDATORY per-verb).
    let err = call_tool(
        &node,
        &principal(ws, &[]),
        ws,
        "inbox.list",
        r#"{"channel":"triage"}"#,
    )
    .await
    .expect_err("inbox.list without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");

    // inbox.resolve (granted) records an approval — the page's first workflow write.
    let resolver = principal(ws, &[INBOX_RESOLVE]);
    call_tool(
        &node,
        &resolver,
        ws,
        "inbox.resolve",
        r#"{"item_id":"i1","decision":"approved","ts":2}"#,
    )
    .await
    .expect("inbox.resolve is granted");
    let res = lb_inbox::resolution(&node.store, ws, "i1")
        .await
        .expect("read resolution")
        .expect("the resolution was written");
    assert_eq!(res.decision, lb_inbox::Decision::Approved);
    assert_eq!(
        res.actor, "user:test",
        "actor forced to the principal's sub"
    );

    // inbox.resolve deny (MANDATORY per-verb).
    let err = call_tool(
        &node,
        &principal(ws, &[]),
        ws,
        "inbox.resolve",
        r#"{"item_id":"i1","decision":"rejected","ts":3}"#,
    )
    .await
    .expect_err("inbox.resolve without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn workflow_surface_is_workspace_isolated() {
    // ws-A is seeded with an inbox item + an outbox effect; ws-B (granted the verbs) sees NONE of it.
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    record(
        &node.store,
        "wf-a",
        &Item::new("i1", "triage", "ext:demo", "secret", 1),
    )
    .await
    .unwrap();
    enqueue(
        &node.store,
        "wf-a",
        "seed_change",
        "e1",
        &serde_json::json!({}),
        &Effect::new("e1", "github", "comment", "x", "e1", 1),
    )
    .await
    .unwrap();

    let b = principal("wf-b", &[INBOX_LIST, OUTBOX]);
    let inbox = call_tool(&node, &b, "wf-b", "inbox.list", r#"{"channel":"triage"}"#)
        .await
        .unwrap();
    let iv: serde_json::Value = serde_json::from_str(&inbox).unwrap();
    assert_eq!(
        iv["items"].as_array().unwrap().len(),
        0,
        "ws-B sees none of ws-A's inbox items — the hard wall, got {iv}"
    );

    let outbox = call_tool(&node, &b, "wf-b", "outbox.status", "{}")
        .await
        .unwrap();
    let ov: serde_json::Value = serde_json::from_str(&outbox).unwrap();
    assert_eq!(
        ov["pending"].as_array().unwrap().len(),
        0,
        "ws-B sees none of ws-A's outbox effects, got {ov}"
    );
}

// ============================================================================================
// host-callback slice (host-callback scope): a wasm GUEST calls host MCP tools through the new
// `host.call-tool` import, under its delegated `caller ∩ install-grant` authority. Every test runs
// through the REAL `lb_runtime` component + real store + real caps (CLAUDE §9).
// ============================================================================================

const DERIVE: &str = "mcp:proof-panel.proof.derive:call";

/// The caps the proof-panel install requests (manifest `[capabilities] request`), as the set a happy
/// install would grant when the admin approves everything. Used to install with a FULL grant so the
/// only variable a deny-test changes is the CALLER's caps (or, inversely, the GRANT's).
fn full_grant() -> Vec<String> {
    [DERIVE, LATEST, WRITE, FIND]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Happy round-trip: a granted caller invokes `proof.derive`; the guest READS the seeded `proof.demo`
/// and WRITES `proof.derived = value*2` — entirely through the host callback. We assert the derived row
/// committed by reading it back over a SEPARATE `series.latest` (the host's store, not the guest's word).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_derive_reads_and_writes_through_the_host_callback() {
    let ws = "cb-happy";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &full_grant(), 1)
        .await
        .expect("install with the full grant");

    // Seed a real source point: proof.demo = 21.
    seed_series(&node, ws, "proof.demo", 7, 21.0).await;

    // The caller holds the derive cap AND the verbs the guest will call back into (the intersection
    // needs BOTH sides to hold them). Invoke the guest tool through the same bridge a page would.
    let caller = principal(ws, &[DERIVE, LATEST, WRITE, FIND]);
    let out = call_tool(&node, &caller, ws, "proof-panel.proof.derive", "{}")
        .await
        .expect("proof.derive runs through the host callback");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["derived"], 42.0, "value*2 = 42, got {v}");

    // The load-bearing assertion: the derived sample really committed to the store — read it back over
    // a SEPARATE series.latest (host-side), not trusting the guest's return value.
    let reader = principal(ws, &[LATEST]);
    let latest = call_tool(
        &node,
        &reader,
        ws,
        "series.latest",
        r#"{"series":"proof.derived"}"#,
    )
    .await
    .expect("read back proof.derived");
    let lv: serde_json::Value = serde_json::from_str(&latest).unwrap();
    assert_eq!(
        lv["sample"]["payload"], 42.0,
        "the guest's callback write is committed in the real store, got {lv}"
    );
}

/// Deny direction (i): the guest calls a verb its INSTALL GRANT omits. The install approves
/// `proof.derive` + `series.latest` but NOT `ingest.write`; the caller DOES hold `ingest.write`. The
/// guest's `ingest.write` callback must be DENIED at the host (delegation narrowing — the grant, not
/// the caller, is the missing side), surfaced as a tool failure, not a silent skip.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn callback_denied_when_install_grant_omits_the_verb() {
    let ws = "cb-grant-deny";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    // Grant omits ingest.write.
    let grant = vec![DERIVE.to_string(), LATEST.to_string(), FIND.to_string()];
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &grant, 1)
        .await
        .expect("install without ingest.write in the grant");
    seed_series(&node, ws, "proof.demo", 7, 21.0).await;

    // The CALLER holds ingest.write — but the intersection narrows it away.
    let caller = principal(ws, &[DERIVE, LATEST, WRITE, FIND]);
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.derive", "{}")
        .await
        .expect_err("the guest's ingest.write callback is denied (grant omits it)");
    // The deny propagates out of the guest as an extension error mentioning the host denial.
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );

    // And nothing was written: proof.derived has no sample.
    let reader = principal(ws, &[LATEST]);
    let latest = call_tool(
        &node,
        &reader,
        ws,
        "series.latest",
        r#"{"series":"proof.derived"}"#,
    )
    .await
    .unwrap();
    let lv: serde_json::Value = serde_json::from_str(&latest).unwrap();
    assert!(
        lv["sample"].is_null(),
        "the denied callback wrote nothing, got {lv}"
    );
}

/// Deny direction (ii): the guest calls a verb the CALLER lacks. The install grant INCLUDES
/// `ingest.write` (the install requested it) but the caller does NOT hold it. The guest's
/// `ingest.write` callback must be DENIED (intersection both ways — the caller, not the grant, is the
/// missing side), even though the install asked for it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn callback_denied_when_caller_lacks_the_verb() {
    let ws = "cb-caller-deny";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    // FULL grant — the install includes ingest.write.
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &full_grant(), 1)
        .await
        .expect("install with the full grant");
    seed_series(&node, ws, "proof.demo", 7, 21.0).await;

    // The caller can invoke derive + read latest, but does NOT hold ingest.write.
    let caller = principal(ws, &[DERIVE, LATEST, FIND]);
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.derive", "{}")
        .await
        .expect_err("the guest's ingest.write callback is denied (caller lacks it)");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );
}

/// Workspace isolation through the callback: a guest invoked in ws-B, calling `series.latest`
/// (`proof.demo`) via the callback, sees NONE of ws-A's data. ws-A is seeded with proof.demo; ws-B is
/// not. The guest in ws-B must fail to derive (no source), proving the host-set ws — never
/// guest-supplied — walls the callback. (If isolation leaked, ws-B's guest would read ws-A's 21.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn callback_is_workspace_isolated() {
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        "iso-a",
        MANIFEST,
        &proof_panel_wasm(),
        &full_grant(),
        1,
    )
    .await
    .expect("install in ws-A");
    install_extension(
        &node,
        "iso-b",
        MANIFEST,
        &proof_panel_wasm(),
        &full_grant(),
        1,
    )
    .await
    .expect("install in ws-B");
    // Only ws-A has the source series.
    seed_series(&node, "iso-a", "proof.demo", 7, 21.0).await;

    // ws-B's guest, fully granted IN ws-B, derives — but there is no proof.demo in ws-B, so the
    // callback's series.latest returns nothing and the guest reports it (it did NOT see ws-A's 21).
    let b = principal("iso-b", &[DERIVE, LATEST, WRITE, FIND]);
    let err = call_tool(&node, &b, "iso-b", "proof-panel.proof.derive", "{}")
        .await
        .expect_err("ws-B has no proof.demo to derive from — the hard wall held");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("proof.demo")),
        "ws-B saw none of ws-A's data, got {err:?}"
    );

    // And ws-A, with its own seeded data, derives fine — proving the wall, not a broken callback.
    let a = principal("iso-a", &[DERIVE, LATEST, WRITE, FIND]);
    let out = call_tool(&node, &a, "iso-a", "proof-panel.proof.derive", "{}")
        .await
        .expect("ws-A derives from its own data");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["derived"], 42.0, "ws-A's own data derives, got {v}");
}

/// Re-entrancy is BOUNDED — never a stack blow-up, never a hang. A guest whose `host.call-tool`
/// re-enters its OWN ext (`proof.recurse` calls `proof-panel.proof.recurse`) would otherwise try to
/// re-lock the single instance its in-flight call already holds. Two host defenses keep it bounded:
///   - the borrow discipline: the callback dispatches through `call_tool`, which `try_lock`s a fresh
///     resolution; a re-entrant self-lock fails fast as "extension busy" rather than deadlocking;
///   - the depth guard (`MAX_CALL_DEPTH`): a cross-instance chain that doesn't self-contend is still
///     capped, returning "call depth exceeded".
/// Either way the call RETURNS an error promptly — the property under test. (The test would HANG, not
/// fail, if re-entrancy were unbounded; reaching the assertion at all proves the bound.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn re_entrancy_is_bounded_never_hangs() {
    const RECURSE: &str = "mcp:proof-panel.proof.recurse:call";
    let ws = "cb-depth";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    // Grant must include the recurse verb on BOTH the install and the caller (the intersection).
    let grant: Vec<String> = full_grant()
        .into_iter()
        .chain([RECURSE.to_string()])
        .collect();
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &grant, 1)
        .await
        .expect("install");
    let caller = principal(ws, &[DERIVE, LATEST, WRITE, FIND, RECURSE]);

    let err = call_tool(&node, &caller, ws, "proof-panel.proof.recurse", "{}")
        .await
        .expect_err(
            "unbounded self-recursion must be stopped (busy or depth-exceeded), never hang",
        );
    assert!(
        matches!(err, ToolError::Extension(ref m)
            if m.contains("extension busy") || m.contains("call depth exceeded")),
        "re-entrancy is bounded promptly (not a hang/overflow), got {err:?}"
    );
}

/// No-identity-leak across calls (the node-global instance hazard): the SAME loaded instance serves
/// two workspaces back to back. Call A runs in ws-A (which has proof.demo); call B runs in ws-B (which
/// does NOT). If identity were instance-sticky, B would inherit A's ws and read A's data. It must not.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn identity_does_not_leak_between_calls_on_the_node_global_instance() {
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        "leak-a",
        MANIFEST,
        &proof_panel_wasm(),
        &full_grant(),
        1,
    )
    .await
    .expect("install ws-A");
    install_extension(
        &node,
        "leak-b",
        MANIFEST,
        &proof_panel_wasm(),
        &full_grant(),
        1,
    )
    .await
    .expect("install ws-B");
    seed_series(&node, "leak-a", "proof.demo", 7, 50.0).await;

    // Call A in ws-A — sets identity for the duration, derives 100, then identity is CLEARED.
    let a = principal("leak-a", &[DERIVE, LATEST, WRITE, FIND]);
    let out_a = call_tool(&node, &a, "leak-a", "proof-panel.proof.derive", "{}")
        .await
        .expect("A derives");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out_a).unwrap()["derived"],
        100.0
    );

    // Call B in ws-B on the SAME node-global instance — must NOT inherit A's ws/data. ws-B has no
    // proof.demo, so it fails to find a source (it did not leak A's 50).
    let b = principal("leak-b", &[DERIVE, LATEST, WRITE, FIND]);
    let err_b = call_tool(&node, &b, "leak-b", "proof-panel.proof.derive", "{}")
        .await
        .expect_err("B must not inherit A's identity/workspace");
    assert!(
        matches!(err_b, ToolError::Extension(ref m) if m.contains("proof.demo")),
        "no identity leak: B saw none of A's data, got {err_b:?}"
    );
}

/// ABI compat: a `@0.1.0` guest (`hello`, built BEFORE this slice — it exports `tool@0.1.0` and
/// imports only `host@0.1.0`'s `log`) STILL loads and answers on a host whose WIT is now `@0.2.0`,
/// SIDE BY SIDE with a `@0.2.0` callback guest. The world MAJOR is unchanged (0), and the runtime
/// links BOTH host-interface versions + falls back to the 0.1.0 export bindings, so the minor bump is
/// backward safe. (Before the compat shim, the 0.1.0 guest failed to instantiate against the 0.2.0
/// linker — see debugging/extensions/wit-minor-bump-breaks-0_1-guest-linking.md.)
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hello_v0_1_guest_still_loads_alongside_a_v0_2_callback_guest() {
    const HELLO_MANIFEST: &str = include_str!("../../../extensions/hello/extension.toml");
    fn hello_wasm() -> Vec<u8> {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../extensions/hello/target/wasm32-wasip2/release/hello_ext.wasm");
        std::fs::read(&path).expect("hello_ext.wasm (build: bash rust/extensions/hello/build.sh)")
    }

    let ws = "abi-compat";
    let node = std::sync::Arc::new(Node::boot().await.unwrap());
    // Load the legacy 0.1.0 guest (no caps requested) AND the 0.2.0 callback guest on the SAME node.
    load_extension(&node, HELLO_MANIFEST, &hello_wasm(), &[])
        .await
        .expect("the @0.1.0 hello guest still loads on a @0.2.0 host");
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &full_grant(), 1)
        .await
        .expect("the @0.2.0 proof-panel callback guest installs");

    // The 0.1.0 guest answers (its `tool@0.1.0` export dispatched through the compat bindings).
    let echoer = principal(ws, &["mcp:hello.echo:call"]);
    let echoed = call_tool(&node, &echoer, ws, "hello.echo", r#"{"msg":"hi"}"#)
        .await
        .expect("the legacy 0.1.0 echo tool still answers");
    let ev: serde_json::Value = serde_json::from_str(&echoed).unwrap();
    assert_eq!(ev["echo"], "hi", "0.1.0 guest round-trips, got {ev}");

    // And the 0.2.0 callback guest works on the same node, proving the two ABI generations coexist.
    seed_series(&node, ws, "proof.demo", 1, 3.0).await;
    let caller = principal(ws, &[DERIVE, LATEST, WRITE, FIND]);
    let out = call_tool(&node, &caller, ws, "proof-panel.proof.derive", "{}")
        .await
        .expect("the 0.2.0 callback guest derives on the same node");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&out).unwrap()["derived"],
        6.0
    );
}

// ============================================================================================
// proof-workflow-sim slice (proof-workflow-sim scope): a wasm GUEST DRIVES a full
// inbox→approval→outbox round-trip through the host callback — it PRODUCES the workflow motion
// (inbox.record → inbox.resolve → outbox.enqueue), instead of only reading something else seeded.
// New write verbs over the bridge: `inbox.record`, `outbox.enqueue`. Real wasm + store + caps.
// ============================================================================================

/// The caps `proof.simulate` exercises end to end: the sim tool itself + every inner workflow verb it
/// calls back into. A FULL grant installs with all of them, so a deny-test changes ONLY one variable.
fn full_sim_grant() -> Vec<String> {
    [
        SIMULATE,
        INBOX_RECORD,
        INBOX_LIST,
        INBOX_RESOLVE,
        OUTBOX_ENQUEUE,
        OUTBOX,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// The caller caps to run `proof.simulate` happily — the same set (the intersection needs BOTH sides).
fn sim_caller_caps() -> Vec<&'static str> {
    vec![
        SIMULATE,
        INBOX_RECORD,
        INBOX_LIST,
        INBOX_RESOLVE,
        OUTBOX_ENQUEUE,
        OUTBOX,
    ]
}

/// Happy round-trip: the guest's `proof.simulate` records an inbox item, resolves it Approved, and
/// enqueues an outbox effect — ALL through the host callback. We assert EACH step via SEPARATE host
/// reads (`inbox.list` / `outbox.status` / the resolution), never the guest's return value.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn proof_simulate_drives_the_full_workflow_round_trip() {
    let ws = "sim-happy";
    let node = Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        ws,
        MANIFEST,
        &proof_panel_wasm(),
        &full_sim_grant(),
        1,
    )
    .await
    .expect("install with the full sim grant");

    let caller = principal(ws, &sim_caller_caps());
    let out = call_tool(&node, &caller, ws, "proof-panel.proof.simulate", "{}")
        .await
        .expect("proof.simulate runs the full round-trip through the callback");
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(
        v["resolved"], true,
        "the guest reports it resolved, got {v}"
    );
    assert_eq!(v["inbox_id"], "proof-sim-item");

    // 1. The inbox item really committed — read it back over a SEPARATE inbox.list.
    let lister = principal(ws, &[INBOX_LIST]);
    let listed = call_tool(
        &node,
        &lister,
        ws,
        "inbox.list",
        r#"{"channel":"proof-triage"}"#,
    )
    .await
    .expect("inbox.list");
    let lv: serde_json::Value = serde_json::from_str(&listed).unwrap();
    let items = lv["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "the guest produced one item, got {lv}");
    assert_eq!(items[0]["id"], "proof-sim-item");
    assert_eq!(
        items[0]["author"], "ext:proof-panel",
        "author is host-forced to the guest's effective principal (the ext acting for the caller), \
         not guest-supplied, got {lv}"
    );

    // 2. The outbox effect really committed pending — read it back over a SEPARATE outbox.status.
    let watcher = principal(ws, &[OUTBOX]);
    let status = call_tool(&node, &watcher, ws, "outbox.status", "{}")
        .await
        .expect("outbox.status");
    let sv: serde_json::Value = serde_json::from_str(&status).unwrap();
    let pending = sv["pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1, "the guest enqueued one effect, got {sv}");
    assert_eq!(pending[0]["id"], "proof-sim-effect");
    assert_eq!(pending[0]["target"], "demo");
}

/// `inbox.record` deny direction (i): the guest calls it but the INSTALL GRANT omits it (caller HOLDS
/// it). The callback must be DENIED — delegation narrowing — surfaced as a guest failure, nothing written.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn simulate_inbox_record_denied_when_grant_omits_it() {
    let ws = "sim-rec-grant-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Grant omits inbox.record.
    let grant: Vec<String> = [SIMULATE, INBOX_LIST, INBOX_RESOLVE, OUTBOX_ENQUEUE, OUTBOX]
        .iter()
        .map(|s| s.to_string())
        .collect();
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &grant, 1)
        .await
        .expect("install without inbox.record in the grant");

    // The CALLER holds inbox.record — but the intersection narrows it away.
    let caller = principal(ws, &sim_caller_caps());
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.simulate", "{}")
        .await
        .expect_err("the guest's inbox.record callback is denied (grant omits it)");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );

    // Nothing was recorded.
    let lister = principal(ws, &[INBOX_LIST]);
    let listed = call_tool(
        &node,
        &lister,
        ws,
        "inbox.list",
        r#"{"channel":"proof-triage"}"#,
    )
    .await
    .unwrap();
    let lv: serde_json::Value = serde_json::from_str(&listed).unwrap();
    assert_eq!(
        lv["items"].as_array().unwrap().len(),
        0,
        "the denied record wrote nothing, got {lv}"
    );
}

/// `inbox.record` deny direction (ii): the guest calls it but the CALLER lacks it (install INCLUDES it).
/// The callback must be DENIED — intersection both ways.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn simulate_inbox_record_denied_when_caller_lacks_it() {
    let ws = "sim-rec-caller-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        ws,
        MANIFEST,
        &proof_panel_wasm(),
        &full_sim_grant(),
        1,
    )
    .await
    .expect("install with the full grant (includes inbox.record)");

    // The caller can invoke simulate but does NOT hold inbox.record.
    let caller = principal(
        ws,
        &[SIMULATE, INBOX_LIST, INBOX_RESOLVE, OUTBOX_ENQUEUE, OUTBOX],
    );
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.simulate", "{}")
        .await
        .expect_err("the guest's inbox.record callback is denied (caller lacks it)");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );
}

/// `outbox.enqueue` deny direction (i): the INSTALL GRANT omits it (caller HOLDS it). The simulation
/// gets past record/resolve, then the enqueue callback is DENIED — and no effect lands.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn simulate_outbox_enqueue_denied_when_grant_omits_it() {
    let ws = "sim-enq-grant-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Grant omits outbox.enqueue.
    let grant: Vec<String> = [SIMULATE, INBOX_RECORD, INBOX_LIST, INBOX_RESOLVE, OUTBOX]
        .iter()
        .map(|s| s.to_string())
        .collect();
    install_extension(&node, ws, MANIFEST, &proof_panel_wasm(), &grant, 1)
        .await
        .expect("install without outbox.enqueue in the grant");

    let caller = principal(ws, &sim_caller_caps());
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.simulate", "{}")
        .await
        .expect_err("the guest's outbox.enqueue callback is denied (grant omits it)");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );

    // No effect was enqueued (the record/resolve before it may have landed; the enqueue did not).
    let watcher = principal(ws, &[OUTBOX]);
    let status = call_tool(&node, &watcher, ws, "outbox.status", "{}")
        .await
        .unwrap();
    let sv: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(
        sv["pending"].as_array().unwrap().len(),
        0,
        "the denied enqueue staged nothing, got {sv}"
    );
}

/// `outbox.enqueue` deny direction (ii): the CALLER lacks it (install INCLUDES it). Denied.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn simulate_outbox_enqueue_denied_when_caller_lacks_it() {
    let ws = "sim-enq-caller-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        ws,
        MANIFEST,
        &proof_panel_wasm(),
        &full_sim_grant(),
        1,
    )
    .await
    .expect("install with the full grant (includes outbox.enqueue)");

    // The caller can invoke simulate but does NOT hold outbox.enqueue.
    let caller = principal(
        ws,
        &[SIMULATE, INBOX_RECORD, INBOX_LIST, INBOX_RESOLVE, OUTBOX],
    );
    let err = call_tool(&node, &caller, ws, "proof-panel.proof.simulate", "{}")
        .await
        .expect_err("the guest's outbox.enqueue callback is denied (caller lacks it)");
    assert!(
        matches!(err, ToolError::Extension(ref m) if m.contains("denied")),
        "guest surfaced the host deny, got {err:?}"
    );
}

/// Workspace isolation: `proof.simulate` in ws-B records/enqueues into ws-B ONLY; a ws-A reader (granted
/// in ws-A) sees NONE of it. The host-set ws — never guest-supplied — walls every callback.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn simulate_is_workspace_isolated() {
    let node = Arc::new(Node::boot().await.unwrap());
    install_extension(
        &node,
        "sim-iso-a",
        MANIFEST,
        &proof_panel_wasm(),
        &full_sim_grant(),
        1,
    )
    .await
    .expect("install ws-A");
    install_extension(
        &node,
        "sim-iso-b",
        MANIFEST,
        &proof_panel_wasm(),
        &full_sim_grant(),
        1,
    )
    .await
    .expect("install ws-B");

    // Run the simulation in ws-B.
    let b = principal("sim-iso-b", &sim_caller_caps());
    call_tool(&node, &b, "sim-iso-b", "proof-panel.proof.simulate", "{}")
        .await
        .expect("ws-B simulates");

    // ws-B sees its own item + effect.
    let b_list = call_tool(
        &node,
        &b,
        "sim-iso-b",
        "inbox.list",
        r#"{"channel":"proof-triage"}"#,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&b_list).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "ws-B has its produced item"
    );

    // ws-A, granted in ws-A, sees NONE of ws-B's produced motion — the hard wall.
    let a = principal("sim-iso-a", &[INBOX_LIST, OUTBOX]);
    let a_list = call_tool(
        &node,
        &a,
        "sim-iso-a",
        "inbox.list",
        r#"{"channel":"proof-triage"}"#,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&a_list).unwrap()["items"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "ws-A sees none of ws-B's inbox items — the hard wall"
    );
    let a_status = call_tool(&node, &a, "sim-iso-a", "outbox.status", "{}")
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&a_status).unwrap()["pending"]
            .as_array()
            .unwrap()
            .len(),
        0,
        "ws-A sees none of ws-B's outbox effects — the hard wall"
    );
}

/// The two new write verbs are gated DIRECTLY over the bridge too (defense in depth, independent of the
/// guest): a caller without the cap is denied; with it, the write lands and a separate read confirms it.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn inbox_record_and_outbox_enqueue_are_gated_over_the_bridge() {
    let ws = "wf-writes";
    let node = Arc::new(Node::boot().await.unwrap());

    // inbox.record deny (no grant) then allow (granted) — confirmed via inbox.list.
    let err = call_tool(
        &node,
        &principal(ws, &[]),
        ws,
        "inbox.record",
        r#"{"channel":"c","id":"x1","body":"hi","ts":1}"#,
    )
    .await
    .expect_err("inbox.record without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");

    let writer = principal(ws, &[INBOX_RECORD, INBOX_LIST]);
    call_tool(
        &node,
        &writer,
        ws,
        "inbox.record",
        r#"{"channel":"c","id":"x1","body":"hi","ts":1}"#,
    )
    .await
    .expect("inbox.record is granted");
    let listed = call_tool(&node, &writer, ws, "inbox.list", r#"{"channel":"c"}"#)
        .await
        .unwrap();
    let lv: serde_json::Value = serde_json::from_str(&listed).unwrap();
    assert_eq!(lv["items"].as_array().unwrap()[0]["id"], "x1");
    assert_eq!(
        lv["items"].as_array().unwrap()[0]["author"],
        "user:test",
        "author host-forced to the principal's sub"
    );

    // outbox.enqueue deny then allow — confirmed via outbox.status.
    let err = call_tool(
        &node,
        &principal(ws, &[]),
        ws,
        "outbox.enqueue",
        r#"{"id":"e1","target":"demo","action":"comment","payload":"p","ts":1}"#,
    )
    .await
    .expect_err("outbox.enqueue without the grant is denied");
    assert!(matches!(err, ToolError::Denied), "opaque deny, got {err:?}");

    let enq = principal(ws, &[OUTBOX_ENQUEUE, OUTBOX]);
    call_tool(
        &node,
        &enq,
        ws,
        "outbox.enqueue",
        r#"{"id":"e1","target":"demo","action":"comment","payload":"p","ts":1}"#,
    )
    .await
    .expect("outbox.enqueue is granted");
    let status = call_tool(&node, &enq, ws, "outbox.status", "{}")
        .await
        .unwrap();
    let sv: serde_json::Value = serde_json::from_str(&status).unwrap();
    assert_eq!(sv["pending"].as_array().unwrap()[0]["id"], "e1");
    assert_eq!(sv["pending"].as_array().unwrap()[0]["target"], "demo");
}
