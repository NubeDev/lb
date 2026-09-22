//! Entity-scoped data (entity-scoped-data scope), end to end: a team handed a menu that marks site A
//! reads ONLY site A's rows through `federation.query` — whatever SQL it sends — while an admin reads
//! everything, and verbs that cannot be narrowed are refused.
//!
//! NO mocks for our own stack: real embedded store, real caps, real nav pick, the REAL supervisor
//! spawning the REAL `federation` sidecar. The external DB is the one sanctioned fake-boundary: a real
//! on-disk SQLite file.

use std::process::Command;
use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, install_native, Node};
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

const MANIFEST: &str = include_str!("../../federation/extension.toml");
const WS: &str = "esrws";

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

fn admin() -> Principal {
    principal(
        "user:admin",
        &[
            "mcp:native.install:call",
            "mcp:native.call:call",
            "mcp:federation.query:call",
            "mcp:federation.row_policy_set:call",
            "mcp:federation.row_policy_get:call",
            "mcp:datasource.add:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
            "mcp:nav.save:call",
            "mcp:nav.share:call",
            "mcp:nav.resolve:call",
            "mcp:teams.manage:call",
            "mcp:teams.create:call",
            "mcp:members.add:call",
        ],
    )
}

/// A group member: the read caps a viewer holds, and no admin marker.
fn member() -> Principal {
    principal(
        "user:m1",
        &["mcp:federation.query:call", "mcp:nav.resolve:call"],
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

/// Two sites, one point each: site A reads 10, site B reads 1000.
fn seed_db(who: &str) -> String {
    // Unique per test: tests are threads of one process (see federation_sqlite_test.rs).
    let path =
        std::env::temp_dir().join(format!("lb-entity-scope-{}-{who}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE point_meta_tags (host_uuid TEXT, point_uuid TEXT, key TEXT, value TEXT);
         CREATE TABLE readings (host_uuid TEXT, point_uuid TEXT, v REAL);
         INSERT INTO point_meta_tags VALUES ('h1','pA','siteRef','Site A'),('h1','pB','siteRef','Site B');
         INSERT INTO readings VALUES ('h1','pA',10.0),('h1','pB',1000.0);",
    )
    .unwrap();
    path.to_string_lossy().into_owned()
}

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    tool: &str,
    input: Value,
) -> Result<Value, lb_mcp::ToolError> {
    let out = call_tool(node, p, WS, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

fn first_number(out: &Value) -> f64 {
    out["rows"][0][0]
        .as_f64()
        .or_else(|| out["rows"][0][0].as_str().and_then(|s| s.parse().ok()))
        .unwrap_or(-1.0)
}

async fn setup(enforce: bool) -> Arc<Node> {
    let dir = federation_dir();
    let db = seed_db(if enforce { "enforced" } else { "open" });
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = admin();
    let approved = vec![
        "net:tls:127.0.0.1:0:connect".to_string(),
        "secret:federation/*:get".to_string(),
    ];
    install_native(&node, &OsLauncher, &admin, WS, MANIFEST, &dir, &approved, 1)
        .await
        .unwrap();
    call(
        &node,
        &admin,
        "datasource.add",
        json!({"name":"esr","kind":"sqlite","endpoint":"127.0.0.1:0","dsn":db,"ts":1}),
    )
    .await
    .unwrap();
    call(
        &node,
        &admin,
        "federation.row_policy_set",
        json!({
            "source": "esr", "enforce": enforce, "entity_table": "site", "scope_sources": ["nav"],
            "policy": {
                "tables": {
                    "point_meta_tags": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]},
                    "readings": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]}
                },
                "entity_key": {"table": "point_meta_tags", "columns": ["host_uuid", "point_uuid"],
                               "key_col": "key", "key_value": "siteRef", "value_col": "value"}
            }
        }),
    )
    .await
    .unwrap();
    // Team g1, member m1, and a menu shared with g1 that marks ONLY site A — nested in a folder, to
    // prove depth does not matter.
    call(
        &node,
        &admin,
        "teams.create",
        json!({"team":"g1","name":"Group 1"}),
    )
    .await
    .unwrap();
    call(
        &node,
        &admin,
        "members.add",
        json!({"team":"g1","user":"user:m1"}),
    )
    .await
    .unwrap();
    call(
        &node,
        &admin,
        "nav.save",
        json!({"id":"g1menu","title":"Group 1","now":1,"items":[
            {"kind":"group","label":"NSW","items":[
                {"kind":"group","label":"Site A","entity":{"table":"site","id":"Site A"},"items":[]}
            ]}
        ]}),
    )
    .await
    .unwrap();
    call(
        &node,
        &admin,
        "nav.share",
        json!({"id":"g1menu","visibility":"team","team":"g1","now":2}),
    )
    .await
    .unwrap();
    node
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_reads_only_the_sites_on_their_handed_menu() {
    let node = setup(true).await;
    let (admin, member) = (admin(), member());
    let q = |sql: &str| json!({"source": "esr", "sql": sql});

    // The same SQL, two answers: the admin sees both sites, the member only site A.
    let total = "SELECT SUM(r.v) FROM readings r JOIN point_meta_tags t ON t.host_uuid = r.host_uuid AND t.point_uuid = r.point_uuid AND t.key = 'siteRef'";
    assert_eq!(
        first_number(
            &call(&node, &admin, "federation.query", q(total))
                .await
                .unwrap()
        ),
        1010.0
    );
    assert_eq!(
        first_number(
            &call(&node, &member, "federation.query", q(total))
                .await
                .unwrap()
        ),
        10.0
    );

    // Asking for site B by hand returns nothing, not an error that confirms it exists.
    let b = call(
        &node,
        &member,
        "federation.query",
        q("SELECT v FROM readings WHERE point_uuid = 'pB'"),
    )
    .await
    .unwrap();
    assert_eq!(b["rows"].as_array().map(Vec::len), Some(0), "{b}");

    // Escapes are refused outright.
    assert!(call(
        &node,
        &member,
        "federation.query",
        q("SELECT query_to_xml('select 1', true, true, '')")
    )
    .await
    .is_err());
    assert!(call(
        &node,
        &member,
        "federation.query",
        q("SELECT * FROM sqlite_master")
    )
    .await
    .is_err());

    // A verb that cannot be narrowed is refused for the member, allowed for the admin.
    let schema = json!({"source": "esr"});
    assert!(call(&node, &member, "federation.schema", schema.clone())
        .await
        .is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unenforced_policy_changes_nothing() {
    let node = setup(false).await;
    let total = json!({"source": "esr", "sql": "SELECT SUM(v) FROM readings"});
    assert_eq!(
        first_number(
            &call(&node, &member(), "federation.query", total)
                .await
                .unwrap()
        ),
        1010.0
    );
}
