//! The S4 skill exit gate, headless: a skill **loads only when granted** by the workspace
//! (README §6.12). The mandatory deny is the with-cap-but-no-grant case (testing §2.1) — a skill
//! that exists is invisible until the workspace grants it, and invisible again after a revoke.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{grant_skill, load_skill, put_skill, revoke_skill, AssetError};
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
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const READ: &str = "store:skill/*:read";
const WRITE: &str = "store:skill/*:write";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn skill_loads_only_when_granted() {
    let ws = "ws-skill-grant";
    let store = Store::memory().await.unwrap();
    let author = principal("user:ada", ws, &[WRITE]);
    let agent = principal("key:agent", ws, &[READ]);

    put_skill(
        &store,
        &author,
        ws,
        "coding-scope-writer",
        "1.0.0",
        "d",
        "BODY",
        1,
    )
    .await
    .unwrap();

    // Before the grant: an agent holding the read cap is DENIED — the §6.12 gate.
    assert!(matches!(
        load_skill(&store, &agent, ws, "coding-scope-writer", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));

    // Workspace grants the skill.
    grant_skill(&store, &author, ws, "coding-scope-writer")
        .await
        .unwrap();

    // Now it loads.
    let s = load_skill(&store, &agent, ws, "coding-scope-writer", None)
        .await
        .unwrap();
    assert_eq!(s.body, "BODY");

    // Revoke makes it invisible again.
    revoke_skill(&store, &author, ws, "coding-scope-writer")
        .await
        .unwrap();
    assert!(matches!(
        load_skill(&store, &agent, ws, "coding-scope-writer", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn without_the_read_cap_a_granted_skill_is_still_denied() {
    // Capability (gate 2) is independent of the grant (gate 3): no `store:skill/*:read` → denied
    // even when the workspace granted the skill.
    let ws = "ws-skill-nocap";
    let store = Store::memory().await.unwrap();
    let author = principal("user:ada", ws, &[WRITE]);
    let nobody = principal("key:nobody", ws, &[]); // no read cap

    put_skill(&store, &author, ws, "s", "1.0.0", "d", "B", 1)
        .await
        .unwrap();
    grant_skill(&store, &author, ws, "s").await.unwrap();

    assert!(matches!(
        load_skill(&store, &nobody, ws, "s", None)
            .await
            .unwrap_err(),
        AssetError::Denied
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn latest_granted_version_loads_and_rollback_pins() {
    let ws = "ws-skill-version";
    let store = Store::memory().await.unwrap();
    let author = principal("user:ada", ws, &[READ, WRITE]);

    put_skill(&store, &author, ws, "s", "1.0.0", "d", "v1", 1)
        .await
        .unwrap();
    put_skill(&store, &author, ws, "s", "1.1.0", "d", "v2", 2)
        .await
        .unwrap();
    grant_skill(&store, &author, ws, "s").await.unwrap();

    // Default load → latest published.
    assert_eq!(
        load_skill(&store, &author, ws, "s", None)
            .await
            .unwrap()
            .body,
        "v2"
    );
    // Pinned (rollback) → the prior version.
    assert_eq!(
        load_skill(&store, &author, ws, "s", Some("1.0.0"))
            .await
            .unwrap()
            .body,
        "v1"
    );
}
