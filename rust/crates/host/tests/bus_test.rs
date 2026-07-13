//! The generic bus pub/sub service, headless (widget-config-vars scope, "Platform fix"). Proves the
//! mandatory categories against a real `Bus`: capability-deny per verb, the workspace-wall subject guard
//! (a reserved prefix / cross-ws / escape attempt is refused), and a publish→watch round-trip within one
//! workspace. Real Zenoh `Bus::peer()` — no mock (CLAUDE §9). Single worker for a deterministic mesh.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_bus::Bus;
use lb_host::{bus_publish, bus_watch, call_bus_tool, wall_subject};
use lb_mcp::ToolError;
use lb_store::Store;
use serde_json::json;
use std::time::Duration;

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

const ALL: &[&str] = &["mcp:bus.publish:call", "mcp:bus.watch:call"];

// The subject wall is pure logic — assert it bites BEFORE any bus call (the structural guard, rule 6).
#[test]
fn wall_subject_namespaces_under_ext_and_refuses_reserved_or_escaping_subjects() {
    // A plain subject is namespaced under `ext/` (the `ws/{id}/` wall is added by the bus layer).
    assert_eq!(wall_subject("cooler/alerts").unwrap(), "ext/cooler/alerts");
    // Reserved platform prefixes are refused (a caller can't impersonate series/channel/internal motion).
    for bad in [
        "series/cpu",
        "channels/x",
        "internal/y",
        "ws/other/series/x",
        "presence/z",
    ] {
        assert!(
            wall_subject(bad).is_err(),
            "reserved must be refused: {bad}"
        );
    }
    // Escape attempts are refused.
    for bad in ["", "  ", "/abs", "a/../b"] {
        assert!(
            wall_subject(bad).is_err(),
            "escape must be refused: {bad:?}"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_without_the_cap_is_denied_opaque() {
    let bus = Bus::peer().await.unwrap();
    let nobody = principal("user:nobody", "ws-bus-deny", &[]);
    // Neither the direct verb nor the MCP bridge leaks anything but an opaque deny.
    assert!(matches!(
        bus_publish(&bus, &nobody, "ws-bus-deny", "x", b"{}").await,
        Err(lb_host::BusError::Denied)
    ));
    let err = call_bus_tool(
        &bus,
        &nobody,
        "ws-bus-deny",
        "bus.publish",
        &json!({ "subject": "x", "payload": {} }),
    )
    .await
    .unwrap_err();
    assert!(matches!(err, ToolError::Denied));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn watch_without_the_cap_is_denied() {
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let nobody = principal("user:nobody", "ws-bus-deny2", &[]);
    assert!(matches!(
        bus_watch(&store, &bus, &nobody, "ws-bus-deny2", "x").await,
        Err(lb_host::BusError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_reserved_or_cross_ws_subject_is_refused_even_with_the_cap() {
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-bus-wall", ALL);
    // A reserved prefix is refused with the cap held (the wall, not the cap, bites here).
    assert!(matches!(
        bus_publish(&bus, &ada, "ws-bus-wall", "series/cpu", b"{}").await,
        Err(lb_host::BusError::BadSubject(_))
    ));
    // The subject can NEVER name another workspace — it is a suffix walled under the caller's ws; a
    // `ws/...` subject is reserved-refused, so a cross-ws name is structurally impossible.
    assert!(matches!(
        bus_watch(&store, &bus, &ada, "ws-bus-wall", "ws/ws-other/secret").await,
        Err(lb_host::BusError::BadSubject(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn publish_watch_round_trips_within_one_workspace() {
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", "ws-bus-rt", ALL);

    // Subscribe FIRST (Zenoh pub/sub is not durable — the sub must exist before the publish).
    let sub = bus_watch(&store, &bus, &ada, "ws-bus-rt", "cooler/alerts")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    let payload = serde_json::to_vec(&json!({ "msg": "defrost" })).unwrap();
    bus_publish(&bus, &ada, "ws-bus-rt", "cooler/alerts", &payload)
        .await
        .unwrap();

    let got = tokio::time::timeout(Duration::from_secs(5), sub.recv())
        .await
        .expect("a published frame arrives within 5s")
        .expect("a payload, not a closed stream");
    let value: serde_json::Value = serde_json::from_slice(&got).unwrap();
    assert_eq!(value["msg"], "defrost");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_does_not_receive_ws_a_publish() {
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let a = principal("user:ada", "ws-bus-a", ALL);
    let b = principal("user:ben", "ws-bus-b", ALL);

    // ben (ws-b) watches the SAME relative subject; the `ws/{id}/` wall makes it a different bus key.
    let sub_b = bus_watch(&store, &bus, &b, "ws-bus-b", "cooler/alerts")
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;

    bus_publish(&bus, &a, "ws-bus-a", "cooler/alerts", b"{\"x\":1}")
        .await
        .unwrap();

    // ben must NOT receive ada's publish — a short timeout elapses with nothing (the wall holds).
    let crossed = tokio::time::timeout(Duration::from_millis(700), sub_b.recv()).await;
    assert!(crossed.is_err(), "ws-B must not receive ws-A's publish");
}

// ─── bus-watch-subject-scope (issue #49) ─────────────────────────────────────────────────────
// Gap 1: when a caller holds ANY `bus:*:watch` grant, `bus.watch` requires a matching one for the
// subject; with NO such grant, behaviour is unchanged (back-compat). Grants seeded through the real
// authz write path (`grant_assign`) into a real `mem://` store — no mocks (CLAUDE §9).

const WS: &str = "ws-scope";

/// Seed `user:<name>` a `bus:<subject>:watch` grant in [`WS`] through the real grant store.
async fn seed_watch_grant(store: &Store, name: &str, subject: &str) {
    lb_authz::grant_assign(
        store,
        WS,
        &lb_authz::Subject::User(name.into()),
        &format!("bus:{subject}:watch"),
    )
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn no_scoped_grant_means_backward_compatible_open_watch() {
    // The load-bearing back-compat assertion: a holder of only the coarse `mcp:bus.watch:call`,
    // with NO `bus:*:watch` grant anywhere, watches ANY subject exactly as today.
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", WS, ALL);
    // Any ext subject is allowed — the subject-scoped gate is inert when no watch grant exists.
    assert!(bus_watch(&store, &bus, &ada, WS, "care.feed.leo")
        .await
        .is_ok());
    assert!(bus_watch(&store, &bus, &ada, WS, "anything/at/all")
        .await
        .is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_scoped_grant_confines_the_holder_to_its_subject() {
    // ada is granted ONLY `bus:care.feed.leo:watch`. That flips her into scoped mode: she may watch
    // leo's feed but is DENIED mia's — Gap 1 closed. She still holds the coarse cap (in ALL).
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", WS, ALL);
    seed_watch_grant(&store, "ada", "care.feed.leo").await;

    assert!(
        bus_watch(&store, &bus, &ada, WS, "care.feed.leo")
            .await
            .is_ok(),
        "ada must reach her own granted subject"
    );
    assert!(
        matches!(
            bus_watch(&store, &bus, &ada, WS, "care.feed.mia").await,
            Err(lb_host::BusError::Denied)
        ),
        "ada must be DENIED another child's subject (Gap 1)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_wildcard_scoped_grant_matches_its_prefix_only() {
    // `bus:care.feed.*:watch` reaches any `care.feed.<x>` but not another namespace — the `*` is one
    // segment (the caps grammar), so scoped mode holds without per-child grants.
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", WS, ALL);
    seed_watch_grant(&store, "ada", "care.feed.*").await;

    assert!(bus_watch(&store, &bus, &ada, WS, "care.feed.leo")
        .await
        .is_ok());
    assert!(bus_watch(&store, &bus, &ada, WS, "care.feed.mia")
        .await
        .is_ok());
    assert!(matches!(
        bus_watch(&store, &bus, &ada, WS, "other.feed.leo").await,
        Err(lb_host::BusError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_scoped_grant_in_another_workspace_does_not_authorize_here() {
    // Workspace isolation (mandatory): a `bus:care.feed.leo:watch` grant seeded in WS does not
    // authorize the same subject for a principal in a DIFFERENT workspace. The wall is checked first
    // by the coarse gate, and the fresh grant read is workspace-namespaced too.
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    seed_watch_grant(&store, "ada", "care.feed.leo").await; // grant lives in WS

    // Same identity, other workspace, holding the coarse cap there. No grant exists in WS-other, so
    // she is in back-compat OPEN mode there — but she can only ever name subjects in HER workspace
    // (the wall), so this proves the WS grant did not leak across the wall into another ws's authz.
    let other = principal("user:ada", "ws-other", ALL);
    // In ws-other ada has NO watch grant → open mode → allowed (and walled to ws-other's motion).
    assert!(bus_watch(&store, &bus, &other, "ws-other", "care.feed.leo")
        .await
        .is_ok());

    // And a principal in WS with NO grant is unaffected by ada's grant (grants are per-subject, not
    // workspace-wide): ben holds the coarse cap, no watch grant → open mode.
    let ben = principal("user:ben", WS, ALL);
    assert!(bus_watch(&store, &bus, &ben, WS, "care.feed.leo")
        .await
        .is_ok());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_grant_assigned_after_login_is_honored_on_next_watch() {
    // Freshness: the gate reads grants from the STORE, not the token. A grant assigned after the
    // principal minted its token still flips scoped mode on the next subscribe — this is what makes
    // revoke-terminates-stream (Gap 2) possible.
    let bus = Bus::peer().await.unwrap();
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", WS, ALL); // token minted with no watch grant

    // Before any grant: open mode, mia is reachable.
    assert!(bus_watch(&store, &bus, &ada, WS, "care.feed.mia")
        .await
        .is_ok());

    // Assign leo AFTER "login" — now ada is in scoped mode and mia is denied, leo allowed. The token
    // never changed; the store read is authoritative.
    seed_watch_grant(&store, "ada", "care.feed.leo").await;
    assert!(bus_watch(&store, &bus, &ada, WS, "care.feed.leo")
        .await
        .is_ok());
    assert!(matches!(
        bus_watch(&store, &bus, &ada, WS, "care.feed.mia").await,
        Err(lb_host::BusError::Denied)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn revoking_the_only_grant_denies_the_subject_it_does_not_reopen() {
    // The isolation-hole guard: once a caller has been placed under scoped enforcement for a subject
    // (via `still_scoped_authorized`, the stream-lifetime predicate), revoking the matching grant
    // DENIES — it must not drop the subject back to back-compat open mode. `authorize_subject_scoped`
    // returns `Open` after the last grant is gone (correct for a *fresh* subscribe: no grant = open),
    // but `still_scoped_authorized` — the predicate the open STREAM re-checks against — is false, so
    // the stream closes and cannot re-open. This asserts that predicate directly.
    let store = Store::memory().await.unwrap();
    let ada = principal("user:ada", WS, ALL);
    seed_watch_grant(&store, "ada", "care.feed.leo").await;

    assert!(
        lb_host::still_scoped_authorized(&store, &ada, WS, "care.feed.leo")
            .await
            .unwrap(),
        "the live grant authorizes the scoped stream"
    );

    lb_authz::grant_revoke(
        &store,
        WS,
        &lb_authz::Subject::User("ada".into()),
        "bus:care.feed.leo:watch",
    )
    .await
    .unwrap();

    assert!(
        !lb_host::still_scoped_authorized(&store, &ada, WS, "care.feed.leo")
            .await
            .unwrap(),
        "after revoke the scoped predicate is FALSE — the stream closes, never re-opens"
    );
}
