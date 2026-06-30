//! access-console scope (mandatory unit, the pure `lb-authz` half): the provenance-tagging wrapper
//! `resolve_caps_sourced` runs the SAME fold as `resolve_caps` (the **resolver↔mint cross-check** —
//! no drift), tags each cap with its source (direct / role / via-team); the `token_revoke` marker
//! reads back; and `role_delete` cascades (un-assigns + drops the role) idempotently. Real store,
//! seeded via the real write path; no mocks.

use std::collections::BTreeSet;

use lb_assets::relate;
use lb_authz::{
    grant_assign, resolve_caps, resolve_caps_sourced, resolve_subject_caps_sourced, role_define,
    role_delete, team_create, token_revoke_mark, token_revoked, CapSource, Subject, MEMBER,
};
use lb_store::Store;

const WS: &str = "acme";

/// Build a workspace where bob has: a DIRECT hvac grant, a `role:auditor` grant (expanding a store
/// cap), and membership in `facilities` (which holds `role:operator`, expanding another store cap).
async fn seed() -> Store {
    let store = Store::memory().await.unwrap();
    role_define(&store, WS, "auditor", &["store:audit/log:read".into()])
        .await
        .unwrap();
    role_define(&store, WS, "operator", &["store:series/hvac:read".into()])
        .await
        .unwrap();
    team_create(&store, WS, "facilities", "Facilities")
        .await
        .unwrap();

    let bob = Subject::User("bob".into());
    // bob's own edges.
    grant_assign(&store, WS, &bob, "mcp:hvac.setpoint:call")
        .await
        .unwrap();
    grant_assign(&store, WS, &bob, "role:auditor")
        .await
        .unwrap();
    // the team's role grant + bob's membership.
    grant_assign(
        &store,
        WS,
        &Subject::Team("facilities".into()),
        "role:operator",
    )
    .await
    .unwrap();
    relate(&store, WS, MEMBER, "facilities", "bob")
        .await
        .unwrap();
    store
}

#[tokio::test]
async fn sourced_cap_set_equals_resolve_caps_no_drift() {
    // THE CROSS-CHECK: the wrapper's cap set MUST equal `resolve_caps(...)` for the same subject —
    // if the provenance fold drifted from the mint fold, an admin would SEE one access set while the
    // gate ENFORCES another (a silent security hole). Pinned.
    let store = seed().await;
    let mut flat: BTreeSet<String> = resolve_caps(&store, WS, "bob")
        .await
        .unwrap()
        .into_iter()
        .collect();
    let sourced: BTreeSet<String> = resolve_caps_sourced(&store, WS, "bob")
        .await
        .unwrap()
        .into_iter()
        .map(|c| c.cap)
        .collect();
    assert_eq!(
        flat, sourced,
        "sourced resolver must not drift from the mint fold"
    );
    // sanity: the expected four caps are present.
    for expected in [
        "mcp:hvac.setpoint:call",
        "store:audit/log:read",
        "store:series/hvac:read",
    ] {
        assert!(flat.remove(expected), "missing {expected}");
    }
}

#[tokio::test]
async fn each_cap_is_tagged_with_the_correct_source() {
    let store = seed().await;
    let caps = resolve_caps_sourced(&store, WS, "bob").await.unwrap();
    let by_cap = |cap: &str| -> Vec<CapSource> {
        caps.iter()
            .find(|c| c.cap == cap)
            .unwrap_or_else(|| panic!("missing {cap}"))
            .source
            .clone()
    };

    // Direct grant → Direct only.
    assert_eq!(by_cap("mcp:hvac.setpoint:call"), vec![CapSource::Direct]);
    // bob's own role → Role(auditor).
    assert_eq!(
        by_cap("store:audit/log:read"),
        vec![CapSource::Role {
            name: "auditor".into()
        }]
    );
    // team-inherited (the team's role grant) → Team(facilities).
    assert_eq!(
        by_cap("store:series/hvac:read"),
        vec![CapSource::Team {
            name: "facilities".into()
        }]
    );
}

#[tokio::test]
async fn subject_caps_sourced_resolves_a_key_with_direct_and_role() {
    // Keys have direct grants + roles but NO team edge; the subject-scoped twin handles them.
    let store = Store::memory().await.unwrap();
    role_define(&store, WS, "auditor", &["store:audit/log:read".into()])
        .await
        .unwrap();
    let key = Subject::Key("k1".into());
    grant_assign(&store, WS, &key, "store:note:read")
        .await
        .unwrap();
    grant_assign(&store, WS, &key, "role:auditor")
        .await
        .unwrap();

    let caps = resolve_subject_caps_sourced(&store, WS, &key)
        .await
        .unwrap();
    let note = caps.iter().find(|c| c.cap == "store:note:read").unwrap();
    assert_eq!(note.source, vec![CapSource::Direct]);
    let audit = caps
        .iter()
        .find(|c| c.cap == "store:audit/log:read")
        .unwrap();
    assert_eq!(
        audit.source,
        vec![CapSource::Role {
            name: "auditor".into()
        }]
    );
}

#[tokio::test]
async fn token_revoke_marker_round_trips_and_is_per_subject() {
    let store = Store::memory().await.unwrap();
    let bob = Subject::User("bob".into());
    let carol = Subject::User("carol".into());

    assert!(
        !token_revoked(&store, WS, &bob).await.unwrap(),
        "absent → not revoked"
    );
    token_revoke_mark(&store, WS, &bob).await.unwrap();
    assert!(
        token_revoked(&store, WS, &bob).await.unwrap(),
        "marked → revoked"
    );
    // per-subject: carol is unaffected.
    assert!(!token_revoked(&store, WS, &carol).await.unwrap());
    // idempotent re-mark.
    token_revoke_mark(&store, WS, &bob).await.unwrap();
    assert!(token_revoked(&store, WS, &bob).await.unwrap());
}

#[tokio::test]
async fn role_delete_cascades_unassign_and_is_idempotent() {
    let store = Store::memory().await.unwrap();
    role_define(&store, WS, "operator", &["store:series/hvac:read".into()])
        .await
        .unwrap();
    // assign the role to two subjects.
    grant_assign(&store, WS, &Subject::User("bob".into()), "role:operator")
        .await
        .unwrap();
    grant_assign(
        &store,
        WS,
        &Subject::Team("facilities".into()),
        "role:operator",
    )
    .await
    .unwrap();

    // delete → 2 subjects un-assigned.
    let affected = role_delete(&store, WS, "operator").await.unwrap();
    assert_eq!(affected, 2);
    // the role is gone → resolves to nothing.
    assert!(lb_authz::role_caps(&store, WS, "operator")
        .await
        .unwrap()
        .is_empty());
    // the assignees' role grants are tombstoned (resolve to no operator caps).
    let bob_caps = resolve_caps(&store, WS, "bob").await.unwrap();
    assert!(!bob_caps.contains(&"store:series/hvac:read".to_string()));

    // idempotent: a repeat delete is a no-op success (0 affected).
    let affected = role_delete(&store, WS, "operator").await.unwrap();
    assert_eq!(affected, 0);
}
