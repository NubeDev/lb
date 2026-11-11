//! Invite accept grants exactly the invite's role. `member` used to be granted on every accept on top
//! of the invite's role, and roles only add, so an invite for `viewer` still produced someone who
//! could author boards and rules. Now `member` is only the fallback for an invite that names no
//! role. Real store, real resolver — no mocks (rule 9).

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_authz::{grant_list, membership_is_member, Subject};
use lb_host::{invite_accept, invite_create};
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

const ADMIN: &[&str] = &[
    "mcp:invite.create:call",
    "mcp:invite.list:call",
    "mcp:grants.assign:call",
    "role:member",
];

/// Invite `email` with `role`, accept it, and return the newcomer's direct grants and live caps.
async fn invite_and_accept(store: &Store, role: &str) -> (Vec<String>, Vec<String>) {
    let key = SigningKey::generate();
    let admin = principal("user:alice", "nube", ADMIN);
    let token = invite_create(
        store,
        &admin,
        "nube",
        "sam@example.com",
        role,
        "",
        None,
        None,
        0,
        100,
    )
    .await
    .expect("invite created");
    let accepted = invite_accept(store, &key, "nube", &token, "password123", None, 200)
        .await
        .expect("invite accepted");
    assert!(
        membership_is_member(store, "nube", "user:sam@example.com")
            .await
            .unwrap(),
        "accept must still join the workspace"
    );
    let grants = grant_list(store, "nube", &Subject::User("sam@example.com".into()))
        .await
        .unwrap();
    (grants, accepted.caps)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_invite_with_a_role_grants_only_that_role() {
    let store = Store::memory().await.unwrap();
    let (grants, caps) = invite_and_accept(&store, "viewer").await;

    assert_eq!(
        grants,
        vec!["role:viewer".to_string()],
        "a viewer invite must not also land role:member"
    );
    let author: Vec<String> = lb_host::author_caps();
    let leaked: Vec<&String> = caps.iter().filter(|c| author.contains(c)).collect();
    assert!(
        leaked.is_empty(),
        "a viewer's first session must hold no member authoring caps: {leaked:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_invite_without_a_role_falls_back_to_member() {
    let store = Store::memory().await.unwrap();
    let (grants, caps) = invite_and_accept(&store, "").await;

    assert_eq!(
        grants,
        vec!["role:member".to_string()],
        "no role on the invite still onboards a working member"
    );
    assert!(!caps.is_empty(), "caps must be live on first login");
}
