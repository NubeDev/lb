//! The i18n-catalog surface through the REAL MCP bridge (`lb_host::call_tool`) — the same entry the
//! gateway's `POST /mcp/call` forwards (i18n-catalogs scope Testing plan, real infra, seeded, no
//! mocks). Covers: per-verb capability deny (incl. render-for-another-recipient without the fan-out
//! grant), two-workspace isolation (a DIFFERENT override in ws-A vs ws-B), offline/replay
//! idempotency, the server-side multi-recipient fan-out headline (2-member team → two distinct
//! renders), and catalog-lint rejection of an out-of-subset override.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{call_tool, Node};
use lb_mcp::ToolError;
use lb_prefs::set_user_prefs;
use lb_prefs::Prefs;
use serde_json::{json, Value};

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

const RENDER: &str = "mcp:message.render:call";
const RENDER_RECIP: &str = "mcp:message.render_recipient:call";
const CATALOG: &str = "mcp:prefs.catalog:call";
const SET_CATALOG: &str = "mcp:message.set_catalog:call";
const SET_PREFS: &str = "mcp:prefs.set:call";

async fn call(
    node: &Arc<Node>,
    p: &Principal,
    ws: &str,
    tool: &str,
    input: Value,
) -> Result<Value, ToolError> {
    let out = call_tool(node, p, ws, tool, &input.to_string()).await?;
    Ok(serde_json::from_str(&out).unwrap())
}

// --- capability deny (mandatory) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn render_denied_without_its_grant() {
    let ws = "cat-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    let nobody = principal("user:eve", ws, &[]);
    let err = call(
        &node,
        &nobody,
        ws,
        "message.render",
        json!({ "key": "notify.welcome", "args": { "name": "X" } }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "render without grant must be denied, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn render_for_another_recipient_denied_without_fanout_grant() {
    let ws = "cat-fanout-deny";
    let node = Arc::new(Node::boot().await.unwrap());
    // Ada may render for HERSELF (base grant) but NOT for another recipient (no fan-out grant).
    let ada = principal("user:ada", ws, &[RENDER]);
    // Self render: allowed.
    call(
        &node,
        &ada,
        ws,
        "message.render",
        json!({ "key": "notify.welcome", "args": { "name": "Ada" } }),
    )
    .await
    .expect("self render allowed with base grant");
    // Render FOR bob: denied (no message.render_recipient).
    let err = call(
        &node,
        &ada,
        ws,
        "message.render",
        json!({ "key": "notify.welcome", "args": { "name": "Ada" }, "recipient": "user:bob" }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::Denied),
        "fan-out without the grant must be denied, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_catalog_denied_for_non_admin() {
    let ws = "cat-setdeny";
    let node = Arc::new(Node::boot().await.unwrap());
    let member = principal("user:bob", ws, &[RENDER, CATALOG]); // no SET_CATALOG
    let err = call(
        &node,
        &member,
        ws,
        "message.set_catalog",
        json!({ "locale": "es", "messages": { "notify.welcome": "Hola {name}" } }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn catalog_read_denied_without_grant_and_cross_ws() {
    let node = Arc::new(Node::boot().await.unwrap());
    let nobody = principal("user:eve", "ws-a", &[]);
    assert!(matches!(
        call(
            &node,
            &nobody,
            "ws-a",
            "prefs.catalog",
            json!({ "locale": "en" })
        )
        .await,
        Err(ToolError::Denied)
    ));
    // A ws-A principal cannot read a foreign workspace (the wall fires first).
    let ada = principal("user:ada", "ws-a", &[CATALOG]);
    assert!(matches!(
        call(
            &node,
            &ada,
            "ws-b",
            "prefs.catalog",
            json!({ "locale": "en" })
        )
        .await,
        Err(ToolError::Denied)
    ));
}

// --- workspace isolation (specified, not generic) ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn two_workspaces_have_distinct_overrides() {
    let node = Arc::new(Node::boot().await.unwrap());
    let admin_a = principal("user:adm", "ws-a", &[SET_CATALOG, CATALOG, RENDER]);
    let admin_b = principal("user:adm", "ws-b", &[SET_CATALOG, CATALOG, RENDER]);

    // The SAME key gets a DIFFERENT override in each workspace.
    call(
        &node,
        &admin_a,
        "ws-a",
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "notify.welcome": "A says hi {name}" } }),
    )
    .await
    .unwrap();
    call(
        &node,
        &admin_b,
        "ws-b",
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "notify.welcome": "B says hi {name}" } }),
    )
    .await
    .unwrap();

    // ws-B render sees ws-B's override, never ws-A's.
    let rb = call(
        &node,
        &admin_b,
        "ws-b",
        "message.render",
        json!({ "key": "notify.welcome", "args": { "name": "Z" } }),
    )
    .await
    .unwrap();
    assert_eq!(rb["text"], "B says hi Z");
    let ra = call(
        &node,
        &admin_a,
        "ws-a",
        "message.render",
        json!({ "key": "notify.welcome", "args": { "name": "Z" } }),
    )
    .await
    .unwrap();
    assert_eq!(ra["text"], "A says hi Z");

    // prefs.catalog is likewise workspace-scoped.
    let cat_b = call(
        &node,
        &admin_b,
        "ws-b",
        "prefs.catalog",
        json!({ "locale": "en" }),
    )
    .await
    .unwrap();
    assert_eq!(cat_b["messages"]["notify.welcome"], "B says hi {name}");
    assert_eq!(cat_b["has_override"], true);
}

// --- offline / sync idempotency ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn override_replays_idempotently_and_merges_per_key() {
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:adm", "ws-off", &[SET_CATALOG, CATALOG]);

    // First edit sets key A.
    call(
        &node,
        &admin,
        "ws-off",
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "notify.welcome": "v1 {name}" } }),
    )
    .await
    .unwrap();
    // The SAME edit replayed (offline replay) — composite id upsert, no duplicate, same result.
    call(
        &node,
        &admin,
        "ws-off",
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "notify.welcome": "v1 {name}" } }),
    )
    .await
    .unwrap();
    // A different-key edit survives alongside A (per-key merge — two offline edits to different keys).
    call(
        &node,
        &admin,
        "ws-off",
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "notify.new_messages": "hi" } }),
    )
    .await
    .unwrap();

    let cat = call(
        &node,
        &admin,
        "ws-off",
        "prefs.catalog",
        json!({ "locale": "en" }),
    )
    .await
    .unwrap();
    assert_eq!(cat["messages"]["notify.welcome"], "v1 {name}");
    assert_eq!(cat["messages"]["notify.new_messages"], "hi");
}

// --- the headline: per-recipient fan-out ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn multi_recipient_fanout_produces_two_distinct_renders() {
    let ws = "cat-team";
    let node = Arc::new(Node::boot().await.unwrap());

    // Seed two members with DIFFERENT resolved prefs through the real write path.
    let ada = principal("user:ada", ws, &[SET_PREFS]);
    let bob = principal("user:bob", ws, &[SET_PREFS]);
    set_user_prefs(
        &node.store,
        ws,
        "user:ada",
        &Prefs {
            language: Some("es".into()),
            timezone: Some("Europe/Madrid".into()),
            date_style: Some(lb_prefs::DateStyle::Eu),
            number_format: Some(lb_prefs::NumberFormat::CommaDot),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    set_user_prefs(
        &node.store,
        ws,
        "user:bob",
        &Prefs {
            language: Some("en".into()),
            timezone: Some("America/New_York".into()),
            date_style: Some(lb_prefs::DateStyle::Usa),
            number_format: Some(lb_prefs::NumberFormat::DotComma),
            unit_overrides: [(lb_prefs::Dimension::Speed, lb_prefs::Unit::Knot)]
                .into_iter()
                .collect(),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    let _ = (ada, bob);

    // The outbox producer holds the fan-out grant and renders once per recipient.
    let producer = principal("svc:outbox", ws, &[RENDER, RENDER_RECIP]);
    let ts_ms = 1_751_373_000_000i64;
    let args = json!({ "name": "Sensor-1", "limit": 12.0, "ts": ts_ms });

    let ra = call(
        &node,
        &producer,
        ws,
        "message.render",
        json!({ "key": "alert.threshold_crossed", "args": args, "recipient": "user:ada" }),
    )
    .await
    .unwrap();
    let rb = call(
        &node,
        &producer,
        ws,
        "message.render",
        json!({ "key": "alert.threshold_crossed", "args": args, "recipient": "user:bob" }),
    )
    .await
    .unwrap();

    // Two DISTINCT renders from one canonical event.
    assert_eq!(ra["locale_used"], "es");
    assert!(
        ra["text"]
            .as_str()
            .unwrap()
            .starts_with("Sensor-1 superó 43,2 km/h el "),
        "es render: {}",
        ra["text"]
    );
    assert_eq!(rb["locale_used"], "en");
    // Bob's knots override + USA date.
    assert!(
        rb["text"]
            .as_str()
            .unwrap()
            .starts_with("Sensor-1 exceeded 23.3 kn at "),
        "en render: {}",
        rb["text"]
    );
    assert_ne!(ra["text"], rb["text"]);
}

// --- catalog-lint rejection at the boundary ---

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_catalog_rejects_out_of_subset_message() {
    let ws = "cat-lint";
    let node = Arc::new(Node::boot().await.unwrap());
    let admin = principal("user:adm", ws, &[SET_CATALOG]);
    // A custom formatter is outside the pinned MF1 subset — a BadInput authoring error, never stored.
    let err = call(
        &node,
        &admin,
        ws,
        "message.set_catalog",
        json!({ "locale": "en", "messages": { "x": "{n, spellout}" } }),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, ToolError::BadInput(_)),
        "out-of-subset override must be BadInput, got {err:?}"
    );
}
