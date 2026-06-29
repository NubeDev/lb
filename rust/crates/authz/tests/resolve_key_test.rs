//! api-keys scope (mandatory unit): `resolve_subject_caps` for a `Subject::Key` resolves its direct
//! grants + role expansion; and the **guard** that the old `resolve_caps(&str)` resolves a key to
//! ZERO caps (the bug this generalized seam exists to avoid — a missed `Key` arm or a key routed
//! through `resolve_caps` would silently deny everything). Real store, seeded via the real write
//! path; no mocks.

use std::collections::BTreeSet;

use lb_authz::{
    grant_assign, grant_revoke, resolve_caps, resolve_subject_caps, role_define, Subject,
};
use lb_store::Store;

// The apikey-read built-in role's caps (mirrors lb_apikey::apikey_read_caps). Inlined here so this
// test stays in the lower-level authz crate with no dependency on lb-apikey.
const READ_CAPS: &[&str] = &["store:*:read", "mcp:*.get:call", "mcp:*.list:call"];

fn read_caps() -> Vec<String> {
    READ_CAPS.iter().map(|s| s.to_string()).collect()
}

#[tokio::test]
async fn a_key_resolves_its_direct_grant_plus_a_role_expansion() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let key = Subject::Key("k7f3a".into());

    role_define(&store, ws, "apikey-read", &read_caps())
        .await
        .unwrap();
    grant_assign(&store, ws, &key, "role:apikey-read")
        .await
        .unwrap();
    grant_assign(&store, ws, &key, "store:series:read")
        .await
        .unwrap();

    let mut caps = BTreeSet::new();
    resolve_subject_caps(&store, ws, &key, &mut caps)
        .await
        .unwrap();
    let resolved: Vec<String> = caps.into_iter().collect();

    for rc in read_caps() {
        assert!(resolved.contains(&rc), "missing role cap {rc}");
    }
    assert!(resolved.contains(&"store:series:read".to_string()));
}

#[tokio::test]
async fn a_key_passed_to_resolve_caps_str_resolves_to_zero_caps() {
    // THE GUARD: resolve_caps wraps its arg in Subject::User, so a key id resolves to nothing. If a
    // future edit routed keys through resolve_caps, this fails-before — keys MUST go through
    // resolve_subject_caps(&Subject::Key).
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let key = Subject::Key("k7f3a".into());
    role_define(&store, ws, "apikey-read", &read_caps())
        .await
        .unwrap();
    grant_assign(&store, ws, &key, "role:apikey-read")
        .await
        .unwrap();

    let via_user_str = resolve_caps(&store, ws, "k7f3a").await.unwrap();
    assert!(
        via_user_str.is_empty(),
        "a key id must NOT resolve through resolve_caps(&str): got {via_user_str:?}"
    );
}

#[tokio::test]
async fn a_revoked_grant_contributes_nothing() {
    let store = Store::memory().await.unwrap();
    let ws = "acme";
    let key = Subject::Key("k1".into());
    grant_assign(&store, ws, &key, "store:note:read")
        .await
        .unwrap();
    grant_revoke(&store, ws, &key, "store:note:read")
        .await
        .unwrap();

    let mut caps = BTreeSet::new();
    resolve_subject_caps(&store, ws, &key, &mut caps)
        .await
        .unwrap();
    assert!(caps.is_empty(), "a tombstoned grant must not resolve");
}
