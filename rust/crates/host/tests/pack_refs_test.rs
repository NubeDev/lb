//! Host-layer tests for entity **source refs** (`entity-source-refs-scope.md`) on a real node —
//! real `mem://` store, real caps, real `pack.validate`/`pack.apply`/`pack.get` verbs, and a real
//! sqlite twin file on disk. No mocks (rule 9).
//!
//! A ref declares that a store-backed entity's rows also exist, under the SAME ids, in a federation
//! datasource. What only a real node can prove, and what this file proves:
//!   - a pack carrying `refs:` (and the `charts.source` they unlock) validates and applies clean;
//!   - `pack.get` hands the block back verbatim — the receipt carriage a downstream surface reads;
//!   - the declared address actually RESOLVES: the `{table, fk}` a receipt carries, applied to the
//!     real sqlite twin, selects the same rows the store holds (id parity is the whole contract);
//!   - a store entity charting a source it declares no ref to GATES at validate (the author's bug);
//!   - a ref to a source this workspace never registered does NOT gate (a workspace fact, resolved
//!     late — gating it would refuse a valid pack on every node but the author's).
//!
//! The federation *read* itself is deliberately not driven here: `federation.query` needs the real
//! federation binary, which is not in the default `cargo test` run (see `pack_store_test.rs`'s module
//! doc). So the twin is read with the same declared address through rusqlite — proving the address
//! resolves to real rows, which is exactly the claim refs make. Core never builds that query either.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use serde_json::{json, Value};

// ----- the pack under test -----------------------------------------------------------------------
//
// `ems_site` lives in the STORE. Its twin lives in the `demo-buildings` datasource: the `site` table
// keyed by the same `id`, and a `point_reading` history keyed by `site_id` — the 15-minute data the
// store seed deliberately does not duplicate. The chart recipe reads that history, which is legal on
// a store entity precisely BECAUSE the ref declares the twin (scope §4).

const REFS_MANIFEST: &str = r#"
pack: refspack
title: Refs Pack
version: 1
entities:
  site:
    label: Site
    table: ems_site
    pk: id
    display: name
    backend: store
    refs:
      - source: demo-buildings
        table: site
        fk: id
        label: Interval data (demo)
      - source: demo-buildings
        table: point_reading
        fk: site_id
    charts:
      - key: demand-hires
        label: Interval demand
        source: demo-buildings
        table: point_reading
        columns: { time: ts, value: val, entity: site_id }
        kind: demand
        window: 7d
seed_rows: seed.json
datasource:
  name: refs-store
  engine: store
"#;

const REFS_SEED: &str = r#"{
  "ems_site": [ {"id":"site-001","name":"Riverside Data Center"},
                {"id":"site-002","name":"Hilltop"} ]
}"#;

fn refs_bundle() -> Value {
    json!({"manifest": REFS_MANIFEST, "files": {"seed.json": REFS_SEED}})
}

// ----- principals / helpers ----------------------------------------------------------------------

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
    "mcp:pack.get:call",
];

/// The full grant a refs-pack applier holds: the pack surface + the store caps the seed and the
/// read-back need. Declaring a ref grants NOTHING extra — no federation cap appears here, and the
/// apply does not need one.
fn refs_full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "store:ems_site:write",
        "mcp:store.write:call",
        "mcp:store.query:call",
    ]);
    principal(ws, &caps)
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

/// The store rows of `table`, unwrapped from the store's `{ data }` envelope (see `pack_store_test`).
async fn store_ids(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    table: &str,
    pk: &str,
) -> Vec<String> {
    let out = call(
        node,
        p,
        ws,
        "store.query",
        json!({"sql": format!("SELECT data FROM {table}")}),
    )
    .await
    .expect("store.query");
    let mut ids: Vec<String> = out["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .filter_map(|r| r["data"][pk].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    ids
}

/// Seed the real sqlite twin: the same site ids the store holds, plus a readings history keyed by
/// `site_id`. Returns the db path.
fn seed_twin_sqlite(tag: &str) -> std::path::PathBuf {
    let db = std::env::temp_dir().join(format!("lb-refs-twin-{}-{tag}.db", std::process::id()));
    let _ = std::fs::remove_file(&db);
    let c = rusqlite::Connection::open(&db).unwrap();
    c.execute_batch(
        "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT);\
         INSERT INTO site VALUES ('site-001','Riverside Data Center');\
         INSERT INTO site VALUES ('site-002','Hilltop');\
         CREATE TABLE point_reading (ts INTEGER, site_id TEXT, val REAL, kind TEXT);\
         INSERT INTO point_reading VALUES (1000,'site-001',42.5,'demand');\
         INSERT INTO point_reading VALUES (2000,'site-001',43.5,'demand');\
         INSERT INTO point_reading VALUES (1000,'site-002',10.0,'demand');",
    )
    .unwrap();
    db
}

/// The receipt's view of one entity, as `pack.get` hands it to a downstream surface.
fn receipt_entity<'a>(got: &'a Value, entity: &str) -> &'a Value {
    let ent = &got["manifest"]["entities"][entity];
    assert!(!ent.is_null(), "pack.get carries the entity: {got}");
    ent
}

// ----- 1. the headline: refs ride the receipt, and the address they carry resolves ----------------

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn refs_ride_the_receipt_and_the_declared_address_resolves_to_the_twin_rows() {
    let ws = "refs-carry";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = refs_full(ws);

    // Validate first — the pack an author would run in CI. Refs + the store-entity chart source they
    // unlock are clean.
    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": refs_bundle()}),
    )
    .await
    .expect("validate runs");
    assert_eq!(out["valid"], true, "a refs pack validates clean: {out}");

    let resp = call(
        &node,
        &p,
        ws,
        "pack.apply",
        json!({"bundle": refs_bundle(), "ts": 10}),
    )
    .await
    .expect("apply");
    assert_eq!(resp["outcome"], "applied", "{resp}");

    // `pack.get` hands the block back VERBATIM — this is the whole downstream contract. A field that
    // applies but does not survive the receipt reaches a consumer as "no refs".
    let got = call(&node, &p, ws, "pack.get", json!({"pack": "refspack"}))
        .await
        .expect("pack.get");
    let site = receipt_entity(&got, "site");
    let refs = site["refs"].as_array().expect("refs array on the receipt");
    assert_eq!(refs.len(), 2, "{site}");
    assert_eq!(refs[0]["source"], "demo-buildings");
    assert_eq!(refs[0]["table"], "site");
    assert_eq!(refs[0]["fk"], "id");
    assert_eq!(refs[0]["label"], "Interval data (demo)");
    assert_eq!(refs[1]["table"], "point_reading");
    assert_eq!(refs[1]["fk"], "site_id");
    // The unlocked chart source rides too — without it the downstream builder cannot know which
    // datasource the recipe addresses.
    assert_eq!(site["charts"][0]["source"], "demo-buildings", "{site}");

    // ID PARITY, proven rather than documented: take the ref's declared `{table, fk}` off the receipt
    // and select the twin rows with it. The ids the store holds are the ids that resolve over there —
    // which is the entire claim a ref makes, and was folklore before this block existed.
    let ids = store_ids(&node, &p, ws, "ems_site", "id").await;
    assert_eq!(ids, vec!["site-001", "site-002"], "store rows seeded");

    let db = seed_twin_sqlite("parity");
    let c = rusqlite::Connection::open(&db).unwrap();
    for (i, expect_rows) in [(0usize, 1usize), (1, 2)] {
        let r = &refs[i];
        let (table, fk) = (
            r["table"].as_str().unwrap(),
            r["fk"].as_str().unwrap_or("id"),
        );
        // The identifiers are validated bare, which is what lets a derived read interpolate them.
        let n: i64 = c
            .query_row(
                &format!("SELECT count(*) FROM {table} WHERE {fk} = ?1"),
                ["site-001"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            n as usize, expect_rows,
            "ref {i} ({table}.{fk}) must resolve site-001's twin rows"
        );
    }
    let _ = std::fs::remove_file(&db);
}

// ----- 2. the unlock's guard: a dangling in-manifest source gates ---------------------------------

/// A **store** entity whose chart names a datasource it declares no ref to is the pack author's bug —
/// readable from the manifest alone, so it gates at validate rather than compiling downstream into a
/// panel that addresses nothing.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_store_chart_source_with_no_declared_ref_gates_at_validate() {
    let ws = "refs-dangling";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    let bundle = json!({
        "manifest": "pack: bad\ntitle: Bad\nversion: 1\n\
            entities:\n  site:\n    label: Site\n    table: ems_site\n    pk: id\n    \
            backend: store\n    charts:\n      - { key: demand, label: Demand, source: demo-buildings }\n",
        "files": {},
    });
    let out = call(&node, &p, ws, "pack.validate", json!({"bundle": bundle}))
        .await
        .expect("validate runs");
    assert_eq!(out["valid"], false, "the dangling source gates: {out}");
    assert!(
        out["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["message"]
                .as_str()
                .unwrap_or("")
                .contains("declares no ref")),
        "the finding names the missing ref: {out}"
    );
}

// ----- 3. the non-gate: an unregistered source is a workspace fact, not a pack defect --------------

/// This node's workspace has never registered `demo-buildings` — and the pack still validates and
/// applies. Source names resolve LATE, per workspace, exactly as every saved `federation.query` cell
/// resolves its own; gating here would refuse a valid pack on every node but the author's. What the
/// viewer gets is honest nothing at read time, not a refused apply.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_ref_to_an_unregistered_source_neither_gates_nor_grants() {
    let ws = "refs-unregistered";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = refs_full(ws);

    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": refs_bundle()}),
    )
    .await
    .expect("validate runs");
    assert_eq!(
        out["valid"], true,
        "an unresolvable ref must not gate: {out}"
    );

    let resp = call(
        &node,
        &p,
        ws,
        "pack.apply",
        json!({"bundle": refs_bundle(), "ts": 10}),
    )
    .await
    .expect("apply");
    assert_eq!(resp["outcome"], "applied", "{resp}");

    // …and it granted nothing: the applier holds no federation cap, and the ref did not conjure one.
    // A ref is an address; reaching the source is still the caps wall's call.
    let denied = call(
        &node,
        &p,
        ws,
        "federation.query",
        json!({"source": "demo-buildings", "sql": "SELECT 1"}),
    )
    .await;
    assert!(
        denied.is_err(),
        "declaring a ref must not grant federation access: {denied:?}"
    );
}

// ----- 4. the payoff, end to end over the REAL federation sidecar ---------------------------------

/// **The claim a ref makes, proven all the way through.** The three tests above stop at the address;
/// this one drives it: apply a pack that ships BOTH a materialized sqlite datasource (the twin, with
/// 15-minute readings) and a store-backed `site` entity declaring `refs:` to it, then build the read
/// a downstream surface would build — `federation.query` over `{ref.source}`/`{ref.table}` filtered by
/// `{ref.fk} = <entity pk>`, taken off `pack.get` — and assert it returns that site's rows. Nothing
/// here is core's work: core carried an address, the caller built the query, which is the whole line
/// this scope holds.
///
/// `#[ignore]` for the same reason as `pack_test.rs`'s O-1 test: it needs the real federation sidecar
/// built. Run with `cargo build -p federation` (or `FEDERATION_BIN=…`) then
/// `cargo test -p lb-host --test pack_refs_test -- --ignored`.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "needs the real federation sidecar built: cargo build -p federation"]
async fn a_ref_derived_federation_query_returns_the_twin_rows() {
    use lb_host::install_native;
    use lb_supervisor::OsLauncher;

    const FEDERATION_MANIFEST: &str = include_str!("../../federation/extension.toml");

    let lb_dir = std::env::temp_dir().join(format!("lb-refs-fed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lb_dir);
    std::env::set_var("LB_DIR", &lb_dir);

    let dir = if let Ok(p) = std::env::var("FEDERATION_BIN") {
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
    };

    // The pack ships BOTH halves the scope describes: a sqlite datasource carrying the high-resolution
    // twin, and a STORE entity whose `refs:` declare that its pk is the key over there.
    let manifest = "\
pack: refsfed
title: Refs Fed
version: 1
entities:
  site:
    label: Site
    table: ems_site
    pk: id
    display: name
    backend: store
    refs:
      - source: demo-twin
        table: point_reading
        fk: site_id
        label: Interval readings (demo)
    charts:
      - key: demand-hires
        label: Interval demand
        source: demo-twin
        table: point_reading
        columns: { time: ts, value: val, entity: site_id }
        kind: demand
        window: 7d
seed_rows: seed.json
datasource:
  name: demo-twin
  engine: sqlite
  schema: schema.sql
  seed: twin.sql
";
    let bundle = json!({
        "manifest": manifest,
        "files": {
            "seed.json": REFS_SEED,
            "schema.sql": "CREATE TABLE point_reading (ts INTEGER, site_id TEXT, val REAL, kind TEXT);",
            "twin.sql": "INSERT INTO point_reading VALUES (1000,'site-001',42.5,'demand');\
                         INSERT INTO point_reading VALUES (2000,'site-001',43.5,'demand');\
                         INSERT INTO point_reading VALUES (1000,'site-002',10.0,'demand');",
        },
    });

    let ws = "refs-fed";
    let node = Arc::new(Node::boot().await.unwrap());
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "store:ems_site:write",
        "mcp:store.write:call",
        "mcp:store.query:call",
        "mcp:native.install:call",
        "mcp:native.call:call",
        "mcp:native.status:call",
        "mcp:datasource.add:call",
        "mcp:datasource.list:call",
        "mcp:federation.query:call",
        "secret:federation/*:write",
        "secret:federation/*:get",
    ]);
    let admin = principal(ws, &caps);

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

    let resp = call(
        &node,
        &admin,
        ws,
        "pack.apply",
        json!({"bundle": bundle, "ts": 10}),
    )
    .await
    .expect("apply");
    assert_eq!(resp["outcome"], "applied", "{resp}");

    // A store entity id — the value a `setVars` pin-click would put in `${site}`.
    let ids = store_ids(&node, &admin, ws, "ems_site", "id").await;
    assert_eq!(
        ids,
        vec!["site-001", "site-002"],
        "store rows seeded: {ids:?}"
    );

    // Build the read the way a downstream surface does: entirely from the receipt's ref, with the
    // entity pk as the parameter. No pack id, no entity key, no recipe survives into it.
    let got = call(&node, &admin, ws, "pack.get", json!({"pack": "refsfed"}))
        .await
        .expect("pack.get");
    let r = &receipt_entity(&got, "site")["refs"][0];
    let (source, table, fk) = (
        r["source"].as_str().unwrap(),
        r["table"].as_str().unwrap(),
        r["fk"].as_str().unwrap_or("id"),
    );
    let out = call(
        &node,
        &admin,
        ws,
        "federation.query",
        json!({
            "source": source,
            "sql": format!("SELECT ts, val FROM {table} WHERE {fk} = 'site-001' ORDER BY ts"),
            "ts": 20,
        }),
    )
    .await
    .expect("the ref-derived federation.query runs");
    let rows = out["rows"].as_array().expect("rows");
    assert_eq!(
        rows.len(),
        2,
        "the store entity's pk selects its twin's rows over in the datasource: {out}"
    );
    assert_eq!(rows[0][0], 1000, "{out}");

    let _ = std::fs::remove_dir_all(&lb_dir);
}
