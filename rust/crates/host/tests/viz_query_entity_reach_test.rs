//! Entity-scoped option sources over `viz.query` (the Forms-10x non-blocking ask). A picker/option
//! target may carry an OPTIONAL `entity` hint — `{ table, cap, pk? }` — so the resolver applies the
//! SAME `authz.scope_filter` (entity-grant reach) the entity's `.list` verb applies to a raw
//! `store.query` on the entity table. Real embedded node, real store-seeded rows, real scoped grants,
//! real caps wall — no mocks (CLAUDE §9). The `ems_site` naming is a test fixture, NOT a core branch
//! (rule 10): the resolver treats `table`/`cap`/`pk` as opaque data.
//!
//! What each test proves:
//!   - `entity_hint_tightens_to_reachable_rows` — a tech scoped to `ems_site:[north]` sees ONLY north
//!     through a raw `store.query` target once the `entity` hint is attached; south is filtered out.
//!   - `entity_hint_all_reach_passes_through` — a principal holding the list cap with `All` scope sees
//!     every row (the hint never narrows a full-reach caller).
//!   - `no_entity_hint_is_unchanged` — the SAME panel WITHOUT the hint returns every row regardless of
//!     grants (additive + opt-in: absent hint ⇒ today's path exactly).
//!   - `hint_on_non_entity_result_degrades_cleanly` — the hint attached to a result that carries no
//!     `pk` column passes rows through unchanged (a mis-hint is inert, never an error or a blank).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, grants_assign, store_write_run, Node, Scope, Subject};
use serde_json::{json, Value};
use std::sync::Arc;

const VIZ: &str = "mcp:viz.query:call";
const QUERY: &str = "mcp:store.query:call";
/// The cap the `ems_site` entity-grant is scoped under — the entity's `.list` verb cap. Opaque to
/// the core; the hint names it so `scope_filter` resolves the right grant.
const SITE_LIST: &str = "mcp:ems.site.list:call";
const SITE_WRITE: &str = "store:ems_site:write";
const ASSIGN: &str = "mcp:grants.assign:call";

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

/// Seed one `ems_site` row (id + name) through the real `store.write` path, at `ems_site:{id}`.
async fn seed_site(node: &Arc<Node>, writer: &Principal, ws: &str, id: &str, name: &str) {
    store_write_run(
        &node.store,
        writer,
        ws,
        "ems_site",
        id,
        &json!({ "id": id, "name": name }),
    )
    .await
    .expect("seed ems_site row");
}

/// A one-target `store.query` picker panel selecting `id, name` from `ems_site`. When `entity` is
/// `Some`, the target carries the reach hint `{ table, cap, pk }`.
fn site_picker_panel(sql: &str, entity: Option<Value>) -> Value {
    let mut src = json!({
        "refId": "A",
        "datasource": { "type": "surreal" },
        "tool": "store.query",
        "args": { "sql": sql },
    });
    if let Some(e) = entity {
        src["entity"] = e;
    }
    json!({ "sources": [src], "transformations": [] })
}

async fn viz_rows(node: &Arc<Node>, p: &Principal, ws: &str, panel: Value) -> Vec<Value> {
    let out = call_tool(
        node,
        p,
        ws,
        "viz.query",
        &json!({ "panel": panel, "now": 1 }).to_string(),
    )
    .await
    .expect("viz.query runs");
    let out: Value = serde_json::from_str(&out).expect("json");
    out["rows"].as_array().cloned().unwrap_or_default()
}

fn names(rows: &[Value]) -> Vec<String> {
    rows.iter()
        .filter_map(|r| r["name"].as_str().map(str::to_string))
        .collect()
}

const SITE_SQL: &str = "SELECT data.id AS id, data.name AS name FROM ems_site ORDER BY data.id";
const ENTITY: fn() -> Value = || json!({ "table": "ems_site", "cap": SITE_LIST, "pk": "id" });

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn entity_hint_tightens_to_reachable_rows() {
    let ws = "viz-ent-reach";
    let node = Arc::new(Node::boot().await.unwrap());

    // Admin seeds two sites and grants the tech reach to ONLY north (a scoped entity-grant).
    let admin = principal("user:admin", ws, &[SITE_WRITE, ASSIGN, SITE_LIST]);
    seed_site(&node, &admin, ws, "north", "North").await;
    seed_site(&node, &admin, ws, "south", "South").await;
    grants_assign(
        &node.store,
        &admin,
        ws,
        &Subject::User("tech".into()),
        SITE_LIST,
        &Scope::Ids {
            table: "ems_site".into(),
            ids: vec!["north".into()],
        },
    )
    .await
    .unwrap();

    // The tech runs the picker WITH the entity hint → only north comes back (its reach), not south.
    let tech = principal("user:tech", ws, &[VIZ, QUERY]);
    let rows = viz_rows(
        &node,
        &tech,
        ws,
        site_picker_panel(SITE_SQL, Some(ENTITY())),
    )
    .await;
    assert_eq!(
        names(&rows),
        vec!["North".to_string()],
        "entity hint tightens the raw store.query to the tech's entity-grant reach"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn entity_hint_all_reach_passes_through() {
    let ws = "viz-ent-all";
    let node = Arc::new(Node::boot().await.unwrap());

    let admin = principal("user:admin", ws, &[SITE_WRITE, ASSIGN, SITE_LIST]);
    seed_site(&node, &admin, ws, "north", "North").await;
    seed_site(&node, &admin, ws, "south", "South").await;
    // A supervisor holds the list cap with FULL reach (Scope::All) — the hint must never narrow it.
    grants_assign(
        &node.store,
        &admin,
        ws,
        &Subject::User("sup".into()),
        SITE_LIST,
        &Scope::All,
    )
    .await
    .unwrap();

    let sup = principal("user:sup", ws, &[VIZ, QUERY]);
    let rows = viz_rows(&node, &sup, ws, site_picker_panel(SITE_SQL, Some(ENTITY()))).await;
    assert_eq!(
        names(&rows),
        vec!["North".to_string(), "South".to_string()],
        "All-reach caller sees every row even with the entity hint"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_entity_hint_is_unchanged() {
    let ws = "viz-ent-none";
    let node = Arc::new(Node::boot().await.unwrap());

    let admin = principal("user:admin", ws, &[SITE_WRITE, ASSIGN, SITE_LIST]);
    seed_site(&node, &admin, ws, "north", "North").await;
    seed_site(&node, &admin, ws, "south", "South").await;
    // The tech is scoped to only north — but WITHOUT the hint the reach filter is never applied.
    grants_assign(
        &node.store,
        &admin,
        ws,
        &Subject::User("tech".into()),
        SITE_LIST,
        &Scope::Ids {
            table: "ems_site".into(),
            ids: vec!["north".into()],
        },
    )
    .await
    .unwrap();

    let tech = principal("user:tech", ws, &[VIZ, QUERY]);
    let rows = viz_rows(&node, &tech, ws, site_picker_panel(SITE_SQL, None)).await;
    assert_eq!(
        names(&rows),
        vec!["North".to_string(), "South".to_string()],
        "no hint ⇒ today's path: every row, reach un-applied (additive/opt-in)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn hint_on_non_entity_result_degrades_cleanly() {
    let ws = "viz-ent-nonent";
    let node = Arc::new(Node::boot().await.unwrap());

    let admin = principal("user:admin", ws, &[SITE_WRITE, ASSIGN, SITE_LIST]);
    seed_site(&node, &admin, ws, "north", "North").await;
    seed_site(&node, &admin, ws, "south", "South").await;
    // Tech scoped to only north — but the picker selects NO pk column (`name` only), so the hint
    // cannot match this result and must pass rows through unchanged (a mis-hint is inert, no error).
    grants_assign(
        &node.store,
        &admin,
        ws,
        &Subject::User("tech".into()),
        SITE_LIST,
        &Scope::Ids {
            table: "ems_site".into(),
            ids: vec!["north".into()],
        },
    )
    .await
    .unwrap();

    let tech = principal("user:tech", ws, &[VIZ, QUERY]);
    let no_pk_sql = "SELECT data.name AS name FROM ems_site ORDER BY data.name";
    let rows = viz_rows(
        &node,
        &tech,
        ws,
        site_picker_panel(no_pk_sql, Some(ENTITY())),
    )
    .await;
    assert_eq!(
        names(&rows),
        vec!["North".to_string(), "South".to_string()],
        "hint on a result with no pk column degrades cleanly — rows unchanged, no error"
    );
}
