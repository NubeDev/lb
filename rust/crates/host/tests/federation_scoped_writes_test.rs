//! Entity-scoped writes (`entity-scoped-writes-scope.md`) through the real verbs: a member whose
//! handed menu marks only Site A writes and deletes Site A's rows of a table the policy marks
//! `writable`, and nothing else — another site's row, a takeover of its key, a table not marked
//! writable — while an admin writes as before.
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

fn admin() -> Principal {
    principal(
        "user:admin",
        &[
            "mcp:native.install:call",
            "mcp:datasource.add:call",
            "mcp:federation.row_policy_set:call",
            "mcp:federation.query:call",
            "mcp:federation.write:call",
            "secret:federation/*:write",
            "secret:federation/*:get",
            "mcp:nav.save:call",
            "mcp:nav.share:call",
            "mcp:teams.manage:call",
            "mcp:teams.create:call",
            "mcp:members.add:call",
        ],
    )
}

/// A site facility manager: the data verbs, no admin marker.
fn member() -> Principal {
    principal(
        "user:m1",
        &[
            "mcp:federation.query:call",
            "mcp:federation.write:call",
            "mcp:federation.delete:call",
            "mcp:nav.resolve:call",
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
    let path =
        std::env::temp_dir().join(format!("lb-scoped-write-{}-{who}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE site_setting (id TEXT PRIMARY KEY, site_id TEXT, value REAL);
         CREATE TABLE site_note (id TEXT PRIMARY KEY, site_id TEXT, note TEXT);
         INSERT INTO site_setting VALUES ('a-kw', 'Site A', 190), ('b-kw', 'Site B', 80);
         INSERT INTO site_note VALUES ('a-n', 'Site A', 'x');",
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

/// Source `cfg` under an enforced policy: `site_setting` writable, `site_note` read-only; member m1's
/// handed menu marks only Site A.
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
    let setup_calls = [
        (
            "datasource.add",
            json!({"name":"cfg","kind":"sqlite","endpoint":"127.0.0.1:0","dsn":db,"ts":1}),
        ),
        (
            "federation.row_policy_set",
            json!({
                "source": "cfg", "enforce": true, "entity_table": "site", "scope_sources": ["nav"],
                "policy": {
                    "tables": {
                        "site_setting": {"kind": "entity", "column": "site_id", "writable": true},
                        "site_note": {"kind": "entity", "column": "site_id"}
                    },
                    "entity_key": {"table": "site_setting", "columns": ["id"], "key_col": "id", "key_value": "-", "value_col": "site_id"}
                }
            }),
        ),
        ("teams.create", json!({"team":"g1","name":"Site A FMs"})),
        ("members.add", json!({"team":"g1","user":"user:m1"})),
        (
            "nav.save",
            json!({"id":"g1menu","title":"Site A","now":1,"items":[
                {"kind":"group","label":"Site A","entity":{"table":"site","id":"Site A"},"items":[]}
            ]}),
        ),
        (
            "nav.share",
            json!({"id":"g1menu","visibility":"team","team":"g1","now":2}),
        ),
    ];
    for (tool, input) in setup_calls {
        call(&node, &admin, tool, input)
            .await
            .unwrap_or_else(|e| panic!("{tool}: {e:?}"));
    }
    node
}

fn save(table: &str, rows: Value) -> Value {
    json!({"source": "cfg", "table": table, "columns": ["id", "site_id", "value"], "rows": rows, "key": ["id"]})
}

/// Every row of `site_setting`, read as the admin (unrestricted).
async fn settings(node: &Arc<Node>) -> Vec<Value> {
    let q =
        json!({"source": "cfg", "sql": "SELECT id, site_id, value FROM site_setting ORDER BY id"});
    let out = call(node, &admin(), "federation.query", q).await.unwrap();
    out["rows"].as_array().unwrap().clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_writes_only_their_own_sites_rows() {
    let node = setup("own").await;
    let m = member();
    call(
        &node,
        &m,
        "federation.write",
        save("site_setting", json!([["a-kw", "Site A", 200.0]])),
    )
    .await
    .expect("own site's row");
    assert!(call(
        &node,
        &m,
        "federation.write",
        save("site_setting", json!([["b-new", "Site B", 1.0]]))
    )
    .await
    .is_err());
    assert!(call(
        &node,
        &m,
        "federation.write",
        save("site_setting", json!([["b-kw", "Site A", 999.0]]))
    )
    .await
    .is_err());
    assert_eq!(
        settings(&node).await,
        [
            json!(["a-kw", "Site A", 200.0]),
            json!(["b-kw", "Site B", 80.0])
        ],
        "own row updated; Site B's row neither created nor taken over"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_member_deletes_only_their_own_sites_rows() {
    let node = setup("delete").await;
    let m = member();
    let del =
        |id: &str| json!({"source": "cfg", "table": "site_setting", "key": ["id"], "rows": [[id]]});
    assert_eq!(
        call(&node, &m, "federation.delete", del("b-kw"))
            .await
            .unwrap()["affected"],
        0
    );
    assert_eq!(
        call(&node, &m, "federation.delete", del("a-kw"))
            .await
            .unwrap()["affected"],
        1
    );
    assert_eq!(settings(&node).await, [json!(["b-kw", "Site B", 80.0])]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_table_not_marked_writable_stays_refused_and_an_admin_writes_as_before() {
    let node = setup("locked").await;
    let note = json!({"source": "cfg", "table": "site_note", "columns": ["id", "site_id", "note"],
                      "rows": [["a-n", "Site A", "y"]], "key": ["id"]});
    let r = call(&node, &member(), "federation.write", note).await;
    assert!(
        matches!(r, Err(ToolError::Denied)),
        "not writable must be denied, got {r:?}"
    );
    call(
        &node,
        &admin(),
        "federation.write",
        save("site_setting", json!([["b-kw", "Site B", 81.0]])),
    )
    .await
    .expect("admin is unrestricted");
    assert!(settings(&node)
        .await
        .contains(&json!(["b-kw", "Site B", 81.0])));
}
