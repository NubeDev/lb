//! Mandatory workspace-isolation for assets at the host layer (testing §2.2): workspace B can
//! never read workspace A's docs/skills — even holding a matching capability whose `ws` claim is
//! B. Gate 1 (workspace isolation) fires before the capability is even consulted (§3.6). This is
//! the store+MCP isolation the S4 prompt requires; the MCP half is in assets_mcp_test.rs.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{get_doc, grant_skill, list_docs, load_skill, put_doc, put_skill, AssetError};
use lb_store::Store;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_read_ws_a_doc() {
    let store = Store::memory().await.unwrap();
    // Ada owns a doc in workspace A.
    let ada_a = principal(
        "user:ada",
        "ws-iso-a",
        &["store:doc/*:read", "store:doc/*:write"],
    );
    put_doc(&store, &ada_a, "ws-iso-a", "scope-x", "T", "secret", 1)
        .await
        .unwrap();

    // The SAME identity, but a token scoped to workspace B, holding a doc read cap. The doc id is
    // identical — only the workspace differs. Gate 1 must refuse before anything is read.
    let ada_b = principal("user:ada", "ws-iso-b", &["store:doc/*:read"]);
    let err = get_doc(&store, &ada_b, "ws-iso-b", "scope-x")
        .await
        .unwrap_err();
    // It does not exist in B → NotFound (still leaks nothing of A); and a cross-ws call (asking
    // for A's ws with a B token) is Denied at gate 1.
    assert!(matches!(err, AssetError::NotFound));

    let cross = principal("user:ada", "ws-iso-b", &["store:doc/*:read"]);
    assert!(matches!(
        get_doc(&store, &cross, "ws-iso-a", "scope-x")
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_list_never_returns_ws_a_docs() {
    let store = Store::memory().await.unwrap();
    let ada_a = principal(
        "user:ada",
        "ws-iso-list-a",
        &["store:doc/*:read", "store:doc/*:write"],
    );
    put_doc(&store, &ada_a, "ws-iso-list-a", "d1", "T", "x", 1)
        .await
        .unwrap();

    let ada_b = principal("user:ada", "ws-iso-list-b", &["store:doc/*:read"]);
    assert!(list_docs(&store, &ada_b, "ws-iso-list-b")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_cannot_load_ws_a_skill() {
    let store = Store::memory().await.unwrap();
    let ada_a = principal(
        "user:ada",
        "ws-iso-skill-a",
        &["store:skill/*:read", "store:skill/*:write"],
    );
    put_skill(&store, &ada_a, "ws-iso-skill-a", "s", "1.0.0", "d", "B", 1)
        .await
        .unwrap();
    grant_skill(&store, &ada_a, "ws-iso-skill-a", "s")
        .await
        .unwrap();

    // A B-scoped token cannot load A's skill (it does not exist in B, and granting/loading in B
    // never sees A's grant). Even reaching across to A's ws is denied at gate 1.
    let ada_b = principal("user:ada", "ws-iso-skill-b", &["store:skill/*:read"]);
    assert!(matches!(
        load_skill(&store, &ada_b, "ws-iso-skill-b", "s", None)
            .await
            .unwrap_err(),
        // No grant in B → Denied (gate 3), and certainly not A's body.
        AssetError::Denied
    ));
    assert!(matches!(
        load_skill(&store, &ada_b, "ws-iso-skill-a", "s", None)
            .await
            .unwrap_err(),
        AssetError::Denied // cross-ws call refused at gate 1
    ));
}
