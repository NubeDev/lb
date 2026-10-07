//! An extension's OWN tables under an enforced row policy (entity-scoped data): an extension acting
//! as itself (`ext:<id>`, no menus, no grants — entity scope "nothing") may create, write and read
//! the tables it created, and read its own store records by id, while every estate table stays
//! narrowed or refused for it, and people are limited exactly as before.
//!
//! Real embedded store, real caps, the REAL supervisor spawning the REAL `federation` sidecar; the
//! external DB is the sanctioned fake-boundary, an on-disk SQLite file.

use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_mcp::ToolError;
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../federation/extension.toml");
const WS: &str = "estate";

fn principal(sub: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: WS.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    verify(&key, &mint(&key, &claims), 1).unwrap()
}

const DATA_CAPS: &[&str] = &[
    "mcp:federation.migrate:call",
    "mcp:federation.write:call",
    "mcp:federation.delete:call",
    "mcp:federation.query:call",
    "mcp:store.get:call",
];

/// An extension backend's token: the data verbs, and its one store table named exactly.
fn extension(id: &str) -> Principal {
    let mut caps = DATA_CAPS.to_vec();
    caps.push("store:waste_config:read");
    principal(&format!("ext:{id}"), &caps)
}

fn admin() -> Principal {
    principal(
        "user:admin",
        &[
            "mcp:native.install:call",
            "mcp:datasource.add:call",
            "mcp:federation.row_policy_set:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
        ],
    )
}

fn federation_dir() -> String {
    if let Ok(p) = std::env::var("FEDERATION_BIN") {
        let dir = std::path::PathBuf::from(&p);
        return dir.parent().unwrap().to_string_lossy().into_owned();
    }
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest_dir.join("../../target/debug");
    let status = Command::new("cargo")
        .args(["build", "-p", "federation"])
        .current_dir(manifest_dir.join("../.."))
        .status()
        .expect("cargo build -p federation runs");
    assert!(status.success() && target.join("federation").exists());
    target.to_string_lossy().into_owned()
}

fn seed_db(who: &str) -> String {
    let path = std::env::temp_dir().join(format!("lb-ext-own-{}-{who}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE point_meta_tags (host_uuid TEXT, point_uuid TEXT, key TEXT, value TEXT);
         CREATE TABLE readings (host_uuid TEXT, point_uuid TEXT, v REAL);
         INSERT INTO point_meta_tags VALUES ('h1','pA','siteRef','Site A');
         INSERT INTO readings VALUES ('h1','pA',10.0);",
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, WS, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

/// A node with source `esr` under an ENFORCED policy over the two estate tables.
async fn setup(who: &str) -> Arc<Node> {
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin();
    let approved = vec![
        "net:tls:127.0.0.1:0:connect".to_string(),
        "secret:federation/*:get".to_string(),
    ];
    install_native(
        &node,
        &OsLauncher,
        &admin,
        WS,
        MANIFEST,
        &federation_dir(),
        &approved,
        1,
    )
    .await
    .unwrap();
    let db = seed_db(who);
    call(
        &node,
        &admin,
        "datasource.add",
        json!({"name":"esr","kind":"sqlite","endpoint":"127.0.0.1:0","dsn":db,"ts":1}),
    )
    .await
    .unwrap();
    call(&node, &admin, "federation.row_policy_set", json!({
        "source": "esr", "enforce": true, "entity_table": "site", "scope_sources": ["nav"], "insight_tag": "site",
        "policy": {
            "tables": {
                "point_meta_tags": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]},
                "readings": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]}
            },
            "entity_key": {"table": "point_meta_tags", "columns": ["host_uuid", "point_uuid"],
                           "key_col": "key", "key_value": "siteRef", "value_col": "value"}
        }
    }))
    .await
    .unwrap();
    node
}

fn design(table: &str) -> Value {
    json!({"tables": [{"name": table, "pk": ["site_ref"], "columns": [
        {"name": "site_ref", "type": "text", "nullable": false},
        {"name": "kg", "type": "real", "nullable": true}
    ]}]})
}

fn migrate(table: &str) -> Value {
    json!({"source": "esr", "schema": design(table), "dry_run": false})
}

fn write(table: &str) -> Value {
    json!({"source": "esr", "table": table, "columns": ["site_ref", "kg"],
           "rows": [["Site A", 2205.0]], "key": ["site_ref"]})
}

fn denied<T: std::fmt::Debug>(r: Result<T, ToolError>, what: &str) {
    assert!(
        matches!(r, Err(ToolError::Denied)),
        "{what} must be denied, got {r:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_extension_creates_writes_and_reads_its_own_table_while_estate_tables_stay_narrowed() {
    let node = setup("own").await;
    let ext = extension("waste");

    call(&node, &ext, "federation.migrate", migrate("waste_entry"))
        .await
        .unwrap();
    // Re-applying the same design plans nothing and is still allowed.
    let again = call(&node, &ext, "federation.migrate", migrate("waste_entry"))
        .await
        .unwrap();
    assert_eq!(again["statements"], json!([]));
    call(&node, &ext, "federation.write", write("waste_entry"))
        .await
        .unwrap();

    let own = call(
        &node,
        &ext,
        "federation.query",
        json!({"source": "esr", "sql": "SELECT site_ref, kg FROM waste_entry"}),
    )
    .await
    .unwrap();
    assert_eq!(own["rows"], json!([["Site A", 2205.0]]));

    // Estate data is still narrowed to the extension's sites: none.
    let estate = call(
        &node,
        &ext,
        "federation.query",
        json!({"source": "esr", "sql": "SELECT value FROM point_meta_tags"}),
    )
    .await
    .unwrap();
    assert_eq!(estate["rows"], json!([]));

    call(
        &node,
        &ext,
        "federation.delete",
        json!({"source": "esr", "table": "waste_entry", "key": ["site_ref"], "rows": [["Site A"]]}),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_extension_cannot_claim_or_change_an_estate_table() {
    let node = setup("estate").await;
    let ext = extension("waste");

    // `readings` already exists: a design naming it is refused, applied or planned.
    denied(
        call(&node, &ext, "federation.migrate", migrate("readings")).await,
        "migrate readings",
    );
    let mut plan = migrate("readings");
    plan["dry_run"] = json!(true);
    denied(
        call(&node, &ext, "federation.migrate", plan).await,
        "plan readings",
    );
    // A design that also names an estate table through an FK is refused as a whole.
    let mut with_fk = migrate("waste_entry");
    with_fk["schema"]["fks"] = json!([{"name": "fk", "from_table": "waste_entry", "from_columns": ["site_ref"],
                                       "to_table": "point_meta_tags", "to_columns": ["value"]}]);
    denied(
        call(&node, &ext, "federation.migrate", with_fk).await,
        "fk to an estate table",
    );

    denied(
        call(&node, &ext, "federation.write", write("readings")).await,
        "write readings",
    );
    denied(
        call(
            &node,
            &ext,
            "federation.delete",
            json!({"source": "esr", "table": "readings", "key": ["host_uuid"], "rows": [["h1"]]}),
        )
        .await,
        "delete readings",
    );
    let read = call(
        &node,
        &ext,
        "federation.query",
        json!({"source": "esr", "sql": "SELECT * FROM waste_entry"}),
    )
    .await;
    assert!(read.is_err(), "a table it never created is not readable");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn another_extension_and_a_restricted_person_are_still_refused_on_that_table() {
    let node = setup("others").await;
    call(
        &node,
        &extension("waste"),
        "federation.migrate",
        migrate("waste_entry"),
    )
    .await
    .unwrap();

    let other = extension("other");
    denied(
        call(&node, &other, "federation.write", write("waste_entry")).await,
        "other ext write",
    );
    denied(
        call(&node, &other, "federation.migrate", migrate("waste_entry")).await,
        "other ext migrate",
    );

    let person = principal("user:m1", DATA_CAPS);
    denied(
        call(&node, &person, "federation.write", write("waste_entry")).await,
        "person write",
    );
    denied(
        call(
            &node,
            &person,
            "federation.migrate",
            migrate("person_table"),
        )
        .await,
        "person migrate",
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn store_get_reads_an_extensions_own_record_and_limits_everyone_else() {
    let node = setup("store").await;
    lb_store::write(
        &node.store,
        WS,
        "waste_config",
        "config",
        &json!({"source": "esr"}),
    )
    .await
    .unwrap();
    let args = json!({"table": "waste_config", "id": "config"});

    let got = call(&node, &extension("waste"), "store.get", args.clone())
        .await
        .unwrap();
    assert_eq!(got["value"], json!({"source": "esr"}));

    // A wildcard is not a table the extension was approved for by name: limited as before.
    let wildcard = principal("ext:wild", &["mcp:store.get:call", "store:*:read"]);
    denied(
        call(&node, &wildcard, "store.get", args.clone()).await,
        "wildcard ext",
    );
    // A restricted person holding the exact cap is limited as on `store.query`.
    let person = principal(
        "user:m1",
        &["mcp:store.get:call", "store:waste_config:read"],
    );
    denied(
        call(&node, &person, "store.get", args.clone()).await,
        "restricted person",
    );
    // No table cap at all: denied by the gate.
    let bare = principal("ext:bare", &["mcp:store.get:call"]);
    denied(call(&node, &bare, "store.get", args).await, "no table cap");
}
