//! Host-layer tests for the `store` datasource kind (`pack-store-datasource-scope.md`, the keystone).
//! Real node (`mem://` store), real caps, real seams — NO mocks and, crucially, NO federation
//! sidecar: a store-engine pack seeds its entity rows straight into the embedded SurrealDB through
//! the same `store.write` verb every workspace record rides, so the whole apply→seed→read-back path
//! runs in-process. That is exactly why these tests ride the DEFAULT `cargo test` run (unlike the
//! sqlite demo oracle, which needs the real federation binary built).
//!
//! What only a real node can prove, and what this file proves:
//!   - a store-engine pack's rows exist as SurrealDB records (`store.query FROM site`) after apply;
//!   - they show in `store.tables` (the Data browser reads this) — "one database";
//!   - a `rel`-style graph edge (`meter.site_id → site`) is followable via `store.query`;
//!   - seed-ownership holds: an operator edit + a new row SURVIVE a re-apply/upgrade (run-once);
//!   - the caps wall re-fires per row: no `store:<table>:write` → the datasource object is `denied`,
//!     an honest partial, never a smuggled write (rule 10 / pack-core no-privileged-path).

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, store_tables_view, Node};
use serde_json::{json, Value};

// ----- the store-backed pack, as a bundle over the wire ------------------------------------------
//
// A pure `store` engine: entity tables `site`/`meter`/`point` bound `backend: store`, seeded from a
// structured `seed.json` (`{table: [rows]}`, O-1 — no SQL). `meter.site_id`/`point.meter_id` make the
// forest a followable graph. No sqlite, no federation source: the rows ARE the store.

const STORE_MANIFEST: &str = "\
pack: bstore
title: Building Store
version: 1
entities:
  site:  { label: Site,  table: site,  pk: id, display: name, backend: store }
  meter: { label: Meter, parent: site,  table: meter, pk: id, parent_fk: site_id,  display: name, backend: store }
  point: { label: Point, parent: meter, table: point, pk: id, parent_fk: meter_id, display: name, backend: store }
seed_rows: seed.json
datasource:
  name: demo-store
  engine: store
";

const STORE_SEED: &str = r#"{
  "site":  [ {"id":"site-001","name":"Riverside","lat":1.0,"lng":2.0},
             {"id":"site-002","name":"Hilltop","lat":3.0,"lng":4.0} ],
  "meter": [ {"id":"meter-001","site_id":"site-001","name":"Main"},
             {"id":"meter-002","site_id":"site-002","name":"Sub"} ],
  "point": [ {"id":"point-001","meter_id":"meter-001","name":"kWh"} ]
}"#;

fn store_bundle_at(version: u32, seed: &str) -> Value {
    let manifest = STORE_MANIFEST.replace("version: 1", &format!("version: {version}"));
    json!({"manifest": manifest, "files": {"seed.json": seed}})
}

fn store_bundle() -> Value {
    store_bundle_at(1, STORE_SEED)
}

// ----- principals --------------------------------------------------------------------------------

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

/// The full grant a store-pack applier holds: the pack surface + the per-table store WRITE caps the
/// seed re-checks per row + the read/browse caps the assertions use. Nothing more.
fn store_full(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "store:site:write",
        "store:meter:write",
        "store:point:write",
        "mcp:store.write:call",
        "mcp:store.delete:call",
        "mcp:store.query:call",
        "mcp:store.tables:call",
        "mcp:store.scan:call",
    ]);
    principal(ws, &caps)
}

/// One grant short: everything EXCEPT `store:meter:write`. The `site` rows seed; the `meter` seed is
/// denied — the datasource object is a partial, not an abort, and not a smuggled write.
fn store_missing_meter_write(ws: &str) -> Principal {
    let mut caps: Vec<&str> = PACK_SURFACE.to_vec();
    caps.extend_from_slice(&[
        "store:site:write",
        "store:point:write",
        "mcp:store.write:call",
        "mcp:store.query:call",
        "mcp:store.tables:call",
    ]);
    principal(ws, &caps)
}

// ----- helpers -----------------------------------------------------------------------------------

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

/// Read the rows of `table` back through the real `store.query` verb, UNWRAPPED from the store's
/// `{ data }` envelope. Store records live as `{ data: <fields>, rev }`, and the raw record `id` is a
/// SurrealDB Thing that does not deserialize to `serde_json::Value` — so we `SELECT data` (the whole
/// row fields) exactly as the EMS extension does (`site_reach/store.rs`), and return the inner objects.
/// This is the same envelope the rubix-ai consumer's store read will unwrap.
async fn read_rows(node: &Arc<Node>, p: &Principal, ws: &str, table: &str) -> Vec<Value> {
    let out = call(
        node,
        p,
        ws,
        "store.query",
        json!({"sql": format!("SELECT data FROM {table}")}),
    )
    .await
    .expect("store.query");
    out["rows"]
        .as_array()
        .map(|rows| rows.iter().filter_map(|r| r.get("data").cloned()).collect())
        .unwrap_or_default()
}

/// Find the unwrapped row of `table` whose `pk` field == `id`.
fn find_row<'a>(rows: &'a [Value], pk: &str, id: &str) -> Option<&'a Value> {
    rows.iter()
        .find(|r| r.get(pk).and_then(Value::as_str) == Some(id))
}

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

// ----- 1. the headline: a store-engine pack seeds SurrealDB records -------------------------------

/// Apply a store-engine pack on a blank workspace → its `site`/`meter`/`point` rows exist as
/// SurrealDB records, readable through `store.query`. No sqlite file, no federation source: the rows
/// ARE the store. This is the whole scope in one assertion — "entities in the store".
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_store_engine_pack_seeds_surrealdb_records() {
    let ws = "store-seed";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = store_full(ws);

    let resp = apply(&node, &p, ws, store_bundle(), 10)
        .await
        .expect("apply the store pack");
    assert_eq!(resp["outcome"], "applied", "every object applied: {resp}");
    assert_eq!(
        object_outcome(&resp, "datasource"),
        "applied",
        "the store datasource seeded cleanly: {resp}"
    );

    // The seeded rows are real SurrealDB records — read them back through store.query.
    let sites = read_rows(&node, &p, ws, "site").await;
    assert_eq!(sites.len(), 2, "two sites seeded: {sites:?}");
    let mut names: Vec<&str> = sites.iter().filter_map(|r| r["name"].as_str()).collect();
    names.sort();
    assert_eq!(
        names,
        vec!["Hilltop", "Riverside"],
        "seeded names: {sites:?}"
    );

    let meters = read_rows(&node, &p, ws, "meter").await;
    assert_eq!(meters.len(), 2, "two meters seeded: {meters:?}");
    let points = read_rows(&node, &p, ws, "point").await;
    assert_eq!(points.len(), 1, "one point seeded: {points:?}");
}

// ----- 2. the rows show in store.tables (the Data browser reads this) ------------------------------

/// A store-backed pack entity is an ORDINARY workspace record: it shows in `store.tables` beside
/// every other table — the "one database" property the Data browser renders. This is what a sqlite
/// pack entity could never do (its rows lived in a private file, invisible here).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seeded_rows_show_in_store_tables() {
    let ws = "store-tables";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = store_full(ws);

    apply(&node, &p, ws, store_bundle(), 10)
        .await
        .expect("apply");

    // `store.tables` is the DB-browser lens (admin-only); call the exported view directly (it is
    // routed at the gateway, not through the generic MCP `call_tool` chain).
    let tables = store_tables_view(&node.store, &p, ws)
        .await
        .expect("store.tables");
    let has = |name: &str| tables.iter().any(|t| t.table == name);
    assert!(has("site"), "site shows in the Data browser: {tables:?}");
    assert!(has("meter"), "meter shows in the Data browser: {tables:?}");
    assert!(has("point"), "point shows in the Data browser: {tables:?}");
}

// ----- 3. a graph edge (meter.site_id → site) is followable ---------------------------------------

/// A store-backed entity carries followable relations: a meter's `site_id` resolves to its site row.
/// This is the `rel`/graph property the scope promises — impossible for a sqlite-file entity.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_meter_to_site_edge_is_followable() {
    let ws = "store-graph";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = store_full(ws);

    apply(&node, &p, ws, store_bundle(), 10)
        .await
        .expect("apply");

    // meter-001 → site-001; read the FK and resolve the site it points at.
    let meters = read_rows(&node, &p, ws, "meter").await;
    let meter = find_row(&meters, "id", "meter-001").expect("meter-001 exists");
    assert_eq!(
        meter["site_id"], "site-001",
        "the meter references its site: {meter}"
    );
    // And the referenced site resolves (the edge target).
    let sites = read_rows(&node, &p, ws, "site").await;
    let site = find_row(&sites, "id", "site-001").expect("the edge target resolves");
    assert_eq!(
        site["name"], "Riverside",
        "the edge target is Riverside: {site}"
    );
}

// ----- 4. seed ownership: an operator edit + a new row survive a re-apply --------------------------

/// The sharp one. Seed rows are STARTING data, applied once. An operator who edits a seeded site and
/// adds their own must keep both across an upgrade (a version bump re-drives every object). The store
/// makes this natural — the seed only fires on FIRST apply, so a re-apply never re-clobbers.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seed_ownership_holds_across_a_reapply() {
    let ws = "store-ownership";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = store_full(ws);

    apply(&node, &p, ws, store_bundle(), 10)
        .await
        .expect("first apply");

    // Operator edits a seeded site's name and adds a brand-new one, via the same store.write verb.
    call(
        &node,
        &p,
        ws,
        "store.write",
        json!({"table":"site","id":"site-001","value":{"id":"site-001","name":"Riverside RENAMED","lat":1.0,"lng":2.0}}),
    )
    .await
    .expect("operator edit");
    call(
        &node,
        &p,
        ws,
        "store.write",
        json!({"table":"site","id":"site-003","value":{"id":"site-003","name":"Operator Site"}}),
    )
    .await
    .expect("operator add");

    // Upgrade the pack (version bump re-drives objects; the seed must NOT re-run).
    let up = apply(&node, &p, ws, store_bundle_at(2, STORE_SEED), 11)
        .await
        .expect("upgrade");
    assert_eq!(up["outcome"], "applied", "the upgrade applied: {up}");

    let sites = read_rows(&node, &p, ws, "site").await;
    let by_id = |id: &str| {
        find_row(&sites, "id", id)
            .and_then(|r| r["name"].as_str())
            .unwrap_or("<absent>")
            .to_string()
    };
    assert_eq!(
        by_id("site-001"),
        "Riverside RENAMED",
        "the operator's EDIT survived the upgrade (seed did not re-clobber): {sites:?}"
    );
    assert_eq!(
        by_id("site-003"),
        "Operator Site",
        "the operator's ADDED row survived the upgrade: {sites:?}"
    );
    assert_eq!(
        sites.len(),
        3,
        "no seed row was resurrected/duplicated: {sites:?}"
    );
}

// ----- 5. the caps wall re-fires per row: no store:<table>:write → a partial ------------------------

/// A pack seeding `site`/`meter`/`point` needs `store:<table>:write` on the applier — re-checked per
/// row under the caller's own principal. A principal missing `store:meter:write` gets the datasource
/// object `denied` (an honest partial), and the `meter` rows are NOT written — a pack grants no
/// privileged path past the same gate a hand `store.write` hits.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_missing_store_write_cap_is_a_denied_partial() {
    let ws = "store-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let short = store_missing_meter_write(ws);

    let resp = apply(&node, &short, ws, store_bundle(), 10)
        .await
        .expect("apply returns a partial, not a transport error");
    assert_eq!(
        resp["outcome"], "partial",
        "one missing store cap makes the apply partial: {resp}"
    );
    assert_eq!(
        object_outcome(&resp, "datasource"),
        "denied",
        "the seed is denied at the meter write it lacks: {resp}"
    );

    // The meter rows were NOT smuggled in — the wall held.
    let meters = read_rows(&node, &short, ws, "meter").await;
    assert!(
        meters.is_empty(),
        "no meter row was written without the cap: {meters:?}"
    );
}

// ----- 5b. the sqlite→store MIGRATION: operator's live rows carried in, no loss, no clobber --------

/// A pack that USED to bind `site` to a sqlite datasource and now binds it `backend: store` names the
/// old datasource in `migrate_from`. A workspace that already CRUD'd the sqlite rows must have the
/// OPERATOR's live rows carried into the store (read the live rows, not the seed) — and the fresh seed
/// must NOT clobber a migrated row that shares an id. This is the sharp "no data loss on upgrade" test
/// (pack-store-datasource-scope §Migration), on a real node with a real sqlite file.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn the_sqlite_to_store_migration_carries_operator_rows_without_loss_or_clobber() {
    // Point the pack db root at a temp dir so the migration's sqlite file never lands in the repo.
    let lb_dir = std::env::temp_dir().join(format!("lb-migrate-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&lb_dir);
    std::env::set_var("LB_DIR", &lb_dir);

    let ws = "store-migrate";
    let pack = "bmig";
    // The prior sqlite db, at the deterministic pack db path (LB_DIR/packs/{ws}/{pack}/{name}.db).
    let old_source = "demo-old";
    let db = lb_dir
        .join("packs")
        .join(ws)
        .join(pack)
        .join(format!("{old_source}.db"));
    std::fs::create_dir_all(db.parent().unwrap()).unwrap();
    {
        let c = rusqlite::Connection::open(&db).unwrap();
        c.execute_batch(
            "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT, lat REAL);\
             INSERT INTO site VALUES ('site-001','Operator RENAMED', 9.9);\
             INSERT INTO site VALUES ('site-op','Operator Added', NULL);",
        )
        .unwrap();
    }

    let node = Arc::new(Node::boot().await.unwrap());
    let p = store_full(ws);

    // The store pack: binds `site` to the store, migrates from the old sqlite, and ships a seed whose
    // `site-001` differs from the operator's edited row (the seed must NOT win over the migration).
    let manifest = format!(
        "pack: {pack}\ntitle: Bmig\nversion: 1\n\
         entities:\n  site: {{ label: Site, table: site, pk: id, display: name, backend: store }}\n\
         seed_rows: seed.json\n\
         migrate_from: {old_source}\n\
         datasource:\n  name: {old_source}\n  engine: store\n"
    );
    // The seed carries site-001 (SHOULD be shadowed by the operator's migrated row) + site-seed (new).
    let seed = r#"{"site":[
        {"id":"site-001","name":"Seed Original","lat":0.0},
        {"id":"site-seed","name":"Seed Only"}
    ]}"#;
    let bundle = json!({"manifest": manifest, "files": {"seed.json": seed}});

    let resp = apply(&node, &p, ws, bundle, 10)
        .await
        .expect("apply w/ migration");
    assert_eq!(resp["outcome"], "applied", "{resp}");

    let sites = read_rows(&node, &p, ws, "site").await;
    let name = |id: &str| {
        find_row(&sites, "id", id)
            .and_then(|r| r["name"].as_str())
            .unwrap_or("<absent>")
            .to_string()
    };
    // The operator's edited row was carried in — and the seed's `site-001` did NOT clobber it.
    assert_eq!(
        name("site-001"),
        "Operator RENAMED",
        "the operator's live sqlite edit wins over the seed: {sites:?}"
    );
    // The operator's ADDED row survived.
    assert_eq!(
        name("site-op"),
        "Operator Added",
        "operator-added row migrated: {sites:?}"
    );
    // A seed id the operator never had is still seeded (site table wasn't empty, so... see note).
    // NOTE: seed-ownership is per-table — because the migration filled `site`, the seed SKIPS the
    // whole table, so `site-seed` is NOT written. That is the correct, conservative behavior: once an
    // operator owns a table's rows, the pack's fresh seed never re-enters it. A pack that needs to add
    // rows on upgrade does so via the entity page, not the seed.
    assert_eq!(
        name("site-seed"),
        "<absent>",
        "seed skips a migrated (owned) table: {sites:?}"
    );
    assert_eq!(sites.len(), 2, "exactly the two operator rows: {sites:?}");

    let _ = std::fs::remove_dir_all(&lb_dir);
}

// ----- 6. a seed_rows table with no bound pk gates at validate ------------------------------------

/// `seed_rows` names a table that no entity binds with a `pk` → a store record has no id column, so
/// the apply cannot key it. This is manifest-only and readable, so it GATES at `pack.validate` (never
/// a silent skip that seeds a pack that is not the one authored).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn seed_rows_with_no_bound_pk_gates_at_validate() {
    let ws = "store-badseed";
    let node = Arc::new(Node::boot().await.unwrap());
    let p = principal(ws, PACK_SURFACE);

    // `orphan` is seeded but no entity binds it — invalid.
    let bundle = json!({
        "manifest": "pack: bad\ntitle: Bad\nversion: 1\n\
            entities:\n  site: { label: Site, table: site, pk: id }\n\
            seed_rows: seed.json\n\
            datasource:\n  name: d\n  engine: store\n",
        "files": {"seed.json": r#"{"site":[{"id":"s1"}],"orphan":[{"id":"o1"}]}"#},
    });

    let out = call(
        &node,
        &p,
        ws,
        "pack.validate",
        json!({"bundle": bundle.clone()}),
    )
    .await
    .expect("validate runs");
    assert_eq!(out["valid"], false, "an unbound seed table gates: {out}");
    assert!(
        out["findings"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["message"].as_str().unwrap_or("").contains("orphan")),
        "the finding names the unbound table: {out}"
    );
}
