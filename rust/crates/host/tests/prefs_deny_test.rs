//! Capability-deny + own-scoping for the prefs surface (prefs scope mandatory category), against a
//! REAL store through the REAL host gate. Seeded real records; no mocks.
//!
//!   - `prefs.set_default` from a non-admin (lacking the admin cap) is DENIED.
//!   - `prefs.get`/`prefs.resolve` are READ OWN: the verb forces the target to the caller's own
//!     `sub`, so a caller can NEVER read another user's record (there is no parameter to name one).
//!   - `prefs.set` writes OWN only — it cannot touch a different user's record.
//!   - A denial is opaque (`PrefsSvcError::Denied`) and leaks nothing — no existence signal.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{prefs_get, prefs_resolve, prefs_set, prefs_set_default, PrefsSvcError};
use lb_prefs::{get_user_prefs, Prefs, UnitSystem};
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

const GET: &str = "mcp:prefs.get:call";
const SET: &str = "mcp:prefs.set:call";
const RESOLVE: &str = "mcp:prefs.resolve:call";
const SET_DEFAULT: &str = "mcp:prefs.set_default:call";

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_default_denied_for_non_admin() {
    let store = Store::memory().await.unwrap();
    // A member with every NON-admin prefs cap but NOT prefs.set_default.
    let member = principal("user:bob", "acme", &[GET, SET, RESOLVE]);
    let err = prefs_set_default(
        &store,
        &member,
        "acme",
        &Prefs {
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
    )
    .await
    .unwrap_err();
    assert!(
        matches!(err, PrefsSvcError::Denied),
        "non-admin set_default must be denied, got {err:?}"
    );

    // And nothing was written (the deny is total, not partial).
    let admin = principal("user:adm", "acme", &[SET_DEFAULT]);
    let resolved = prefs_resolve_unchecked(&store, &admin).await;
    assert_eq!(
        resolved,
        UnitSystem::Metric,
        "the denied default did not land"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn get_reads_only_callers_own_record() {
    let store = Store::memory().await.unwrap();
    // Ada seeds her own prefs (metric). Bob seeds his (imperial).
    let ada = principal("user:ada", "acme", &[GET, SET]);
    let bob = principal("user:bob", "acme", &[GET, SET]);
    prefs_set(
        &store,
        &ada,
        "acme",
        &Prefs {
            unit_system: Some(UnitSystem::Metric),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();
    prefs_set(
        &store,
        &bob,
        "acme",
        &Prefs {
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    // Bob's get returns BOB's record — there is no way for him to address Ada's.
    let bob_got = prefs_get(&store, &bob, "acme").await.unwrap().unwrap();
    assert_eq!(bob_got.unit_system, Some(UnitSystem::Imperial));

    // Confirm the records are genuinely distinct in the store (so the own-scoping is meaningful).
    let ada_raw = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ada_raw.unit_system, Some(UnitSystem::Metric));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn set_cannot_write_another_users_record() {
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "acme", &[SET]);
    // Ada sets prefs — forced to her own sub. Even if a malicious patch tried to carry a `user`
    // field, the verb ignores it (the target is `principal.sub()`).
    prefs_set(
        &store,
        &ada,
        "acme",
        &Prefs {
            unit_system: Some(UnitSystem::Imperial),
            ..Prefs::default()
        },
    )
    .await
    .unwrap();

    // Bob's record is untouched (he never set anything; still None).
    let bob = get_user_prefs(&store, "acme", "user:bob").await.unwrap();
    assert!(
        bob.is_none(),
        "Ada's set must not have created/written Bob's record"
    );
    // Ada's own record landed.
    let ada_rec = get_user_prefs(&store, "acme", "user:ada")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(ada_rec.unit_system, Some(UnitSystem::Imperial));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn deny_without_any_cap_is_opaque() {
    let store = Store::memory().await.unwrap();
    let nobody = principal("user:eve", "acme", &[]); // no prefs caps at all
    assert!(matches!(
        prefs_get(&store, &nobody, "acme").await,
        Err(PrefsSvcError::Denied)
    ));
    assert!(matches!(
        prefs_resolve(&store, &nobody, "acme", None).await,
        Err(PrefsSvcError::Denied)
    ));
    assert!(matches!(
        prefs_set(&store, &nobody, "acme", &Prefs::default()).await,
        Err(PrefsSvcError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn cross_workspace_resolve_is_denied() {
    // A principal scoped to ws-A cannot resolve in ws-B — gate 1 (the hard wall) fires first.
    let store = Store::memory().await.unwrap();
    let ada_a = principal("user:ada", "ws-a", &[RESOLVE]);
    let err = prefs_resolve(&store, &ada_a, "ws-b", None)
        .await
        .unwrap_err();
    assert!(matches!(err, PrefsSvcError::Denied));
}

/// Helper: resolve `admin`'s prefs and return the resolved unit_system (used to assert a denied
/// `set_default` left the workspace default unchanged).
async fn prefs_resolve_unchecked(store: &Store, admin: &Principal) -> UnitSystem {
    // Grant the admin a resolve cap implicitly by going through the raw chain (no auth needed for
    // the assertion — we only want the resolved value the store would yield).
    lb_prefs::resolve_chain(store, admin.ws(), admin.sub(), None)
        .await
        .unwrap()
        .unit_system
}
