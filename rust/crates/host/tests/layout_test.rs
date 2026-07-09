//! The ui-layout verbs, headless (data-studio scope v2, "Layout persistence"). Proves the mandatory
//! categories against a real store: the get/set round-trip (LWW upsert), capability-deny per verb,
//! two-workspace isolation, the MEMBER-OWNED keying (two users on the same surface never see each
//! other's layout — the record is keyed to the principal's `sub`, never an argument), the per-surface
//! keying, the size bound (reject, don't truncate), and the empty-surface `BadInput`.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{layout_get, layout_set, LayoutError, MAX_LAYOUT_BYTES};
use lb_store::Store;
use serde_json::json;

/// A principal `sub` in workspace `ws` holding `caps`.
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

const GET: &str = "mcp:layout.get:call";
const SET: &str = "mcp:layout.set:call";
const ALL: &[&str] = &[GET, SET];

#[tokio::test]
async fn round_trip_and_lww_upsert() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);

    // Absent → a default record (empty model), not an error.
    let empty = layout_get(&store, &ada, "ws-a", "data-studio")
        .await
        .unwrap();
    assert_eq!(empty.model, serde_json::Value::Null);
    assert_eq!(empty.surface, "data-studio");

    // Set → get returns the same model; a second set wins (LWW).
    let m1 = json!({ "layout": { "type": "row", "children": [] }, "rev": 1 });
    let saved = layout_set(&store, &ada, "ws-a", "data-studio", m1.clone(), 100)
        .await
        .unwrap();
    assert_eq!(saved.model, m1);
    assert_eq!(saved.updated_ts, 100);
    let got = layout_get(&store, &ada, "ws-a", "data-studio")
        .await
        .unwrap();
    assert_eq!(got.model, m1);

    let m2 = json!({ "rev": 2 });
    layout_set(&store, &ada, "ws-a", "data-studio", m2.clone(), 200)
        .await
        .unwrap();
    let got = layout_get(&store, &ada, "ws-a", "data-studio")
        .await
        .unwrap();
    assert_eq!(got.model, m2);
    assert_eq!(got.updated_ts, 200);
}

#[tokio::test]
async fn member_owned_two_users_never_cross() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);
    let ben = principal("user:ben", "ws-a", ALL);

    layout_set(
        &store,
        &ada,
        "ws-a",
        "data-studio",
        json!({"who": "ada"}),
        1,
    )
    .await
    .unwrap();

    // Ben, same workspace + same surface, sees HIS OWN (absent) layout — never Ada's. The record is
    // keyed to the token `sub`; there is no argument through which Ben could name Ada.
    let bens = layout_get(&store, &ben, "ws-a", "data-studio")
        .await
        .unwrap();
    assert_eq!(bens.model, serde_json::Value::Null);

    layout_set(
        &store,
        &ben,
        "ws-a",
        "data-studio",
        json!({"who": "ben"}),
        2,
    )
    .await
    .unwrap();
    let adas = layout_get(&store, &ada, "ws-a", "data-studio")
        .await
        .unwrap();
    assert_eq!(
        adas.model,
        json!({"who": "ada"}),
        "ben's write must not clobber ada's"
    );
}

#[tokio::test]
async fn per_surface_keying() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);

    layout_set(&store, &ada, "ws-a", "data-studio", json!({"s": 1}), 1)
        .await
        .unwrap();
    layout_set(&store, &ada, "ws-a", "flows", json!({"s": 2}), 2)
        .await
        .unwrap();

    assert_eq!(
        layout_get(&store, &ada, "ws-a", "data-studio")
            .await
            .unwrap()
            .model,
        json!({"s": 1})
    );
    assert_eq!(
        layout_get(&store, &ada, "ws-a", "flows")
            .await
            .unwrap()
            .model,
        json!({"s": 2})
    );
}

#[tokio::test]
async fn workspace_isolation() {
    let store = Store::memory().await.unwrap();
    let ada_a = principal("user:ada", "ws-a", ALL);
    let ada_b = principal("user:ada", "ws-b", ALL);

    layout_set(&store, &ada_a, "ws-a", "data-studio", json!({"ws": "a"}), 1)
        .await
        .unwrap();

    // Same user, different workspace: the hard wall — nothing crosses.
    let in_b = layout_get(&store, &ada_b, "ws-b", "data-studio")
        .await
        .unwrap();
    assert_eq!(in_b.model, serde_json::Value::Null);

    // A ws-A token calling with ws-B is refused before the verb runs (workspace-first).
    let denied = layout_get(&store, &ada_a, "ws-b", "data-studio").await;
    assert!(matches!(denied, Err(LayoutError::Denied)));
}

#[tokio::test]
async fn capability_deny_per_verb() {
    let store = Store::memory().await.unwrap();
    let only_get = principal("user:ada", "ws-a", &[GET]);
    let only_set = principal("user:ben", "ws-a", &[SET]);
    let none = principal("user:cat", "ws-a", &[]);

    // Missing `layout.set` → set denied (opaque), even with the read grant.
    let denied = layout_set(&store, &only_get, "ws-a", "data-studio", json!({}), 1).await;
    assert!(matches!(denied, Err(LayoutError::Denied)));

    // Missing `layout.get` → get denied, even with the write grant.
    let denied = layout_get(&store, &only_set, "ws-a", "data-studio").await;
    assert!(matches!(denied, Err(LayoutError::Denied)));

    let denied = layout_get(&store, &none, "ws-a", "data-studio").await;
    assert!(matches!(denied, Err(LayoutError::Denied)));
}

#[tokio::test]
async fn bounds_and_bad_input() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-a", ALL);

    // An over-cap model is rejected loudly (never truncated).
    let big = json!({ "blob": "x".repeat(MAX_LAYOUT_BYTES) });
    let denied = layout_set(&store, &ada, "ws-a", "data-studio", big, 1).await;
    assert!(matches!(denied, Err(LayoutError::BadInput(_))));

    // An empty surface key is bad input on both verbs.
    assert!(matches!(
        layout_get(&store, &ada, "ws-a", "").await,
        Err(LayoutError::BadInput(_))
    ));
    assert!(matches!(
        layout_set(&store, &ada, "ws-a", "", json!({}), 1).await,
        Err(LayoutError::BadInput(_))
    ));
}
