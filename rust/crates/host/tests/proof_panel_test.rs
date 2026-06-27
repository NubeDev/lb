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
    Node, Qos, Sample,
};
use lb_mcp::ToolError;
use lb_registry::{digest, digest_hex, Artifact, PublisherKey, TrustedKeys, Visibility};
use lb_tags::{Provenance, Source, Tag, DEFAULT_TAG_NODE_CAP};

const MANIFEST: &str = include_str!("../../../extensions/proof-panel/extension.toml");

const PING: &str = "mcp:proof-panel.proof.ping:call";
const PUBLISH: &str = "mcp:ext.publish:call";
const FIND: &str = "mcp:series.find:call";
const LATEST: &str = "mcp:series.latest:call";

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
    let node = Node::boot().await.unwrap();
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
    let node = Node::boot().await.unwrap();
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
    let node = Node::boot().await.unwrap();
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
    let node = Node::boot().await.unwrap();
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
