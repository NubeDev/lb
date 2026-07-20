//! `POST /mcp/call` — the optional `node` addressing axis (routed-dispatch-sidecar-bridge scope).
//!
//! Routed dispatch (#81) shipped the engine, but no HTTP caller could reach it: the bridge body was
//! `{tool, args}` with no axis to name a target node, so a native sidecar could be TOLD a call was
//! `Ambiguous` (409) but had no way to answer. This file covers the request axis that closes that
//! gap, at the HTTP boundary a sidecar actually speaks.
//!
//! The routing itself (does a targeted call land on the node named, and never fall back?) is proven
//! against two real Zenoh-linked nodes in `lb-host`'s `routed_host_entry_test.rs` — that is the
//! determinism guard, and it is mutation-checked. What is proven HERE is what only the bridge can
//! decide:
//!   - a `node` field is **parsed and threaded**, not ignored;
//!   - a malformed node id is **`400 BadInput`**, never `403` — it is author feedback about the
//!     call's shape, and collapsing it to `403` would read as a capability denial and hide the typo
//!     (a `SidecarClient` maps 403 → `Denied`, so this exact confusion has bitten before);
//!   - **omitting `node` is byte-for-byte today's behaviour** — the backwards-compat guarantee the
//!     wide `call_tool` fan-out depends on;
//!   - **addressing is not authorization (mandatory deny-test):** naming a node cannot widen what a
//!     caller may do, and the deny carries no existence signal about the node named.
//!
//! Real gateway, real router, real token verification, real capability checks — no mocks (rule 9).

mod common;

use axum::http::StatusCode;
use common::*;
use lb_role_gateway::router;
use tower::ServiceExt; // for `oneshot`

/// A bridged call body. `node` is included only when `Some` — so the no-node case produces exactly
/// the pre-scope body shape (the backwards-compat assertion is on the wire, not just in spirit).
fn call_body(tool: &str, node: Option<&str>) -> serde_json::Value {
    match node {
        Some(n) => serde_json::json!({ "tool": tool, "args": {}, "node": n }),
        None => serde_json::json!({ "tool": tool, "args": {} }),
    }
}

/// A malformed node id is author feedback (`400`), NOT an authorization signal (`403`). `*` is
/// key-unsafe (it is a Zenoh wildcard — accepting it would let one call address a whole fleet), so
/// `NodeId::new` rejects it and the bridge must surface that as a bad request.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_malformed_node_id_is_a_400_not_a_403() {
    let (gw, key) = gateway().await;
    // A caller WITH the capability — so a 403 here could only mean the bad id was misclassified as
    // a denial, which is precisely the confusion this asserts against.
    let tok = token(&key, "user:a", "ws-a", &["mcp:host.time.now:call"]);

    for bad in ["gw-*", "", "node:a/b"] {
        let resp = router(gw.clone())
            .oneshot(bearer(
                json_post("/mcp/call", call_body("host.time.now", Some(bad))),
                &tok,
            ))
            .await
            .unwrap();

        assert_eq!(
            resp.status(),
            StatusCode::BAD_REQUEST,
            "a malformed node id ({bad:?}) must be 400 BadInput — as author feedback about the \
             call's shape. A 403 would read as a capability denial and hide the typo."
        );
    }
}

/// Omitting `node` must behave EXACTLY as before the axis existed. This is the guarantee that lets
/// the many existing `call_tool` callers (agent loop, gateway routes, reach path) stay untouched:
/// a body with no `node` deserializes fine (`#[serde(default)]`) and dispatches untargeted.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_body_without_node_still_dispatches_exactly_as_before() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:a", "ws-a", &["mcp:host.time.now:call"]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/mcp/call", call_body("host.time.now", None)),
            &tok,
        ))
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "an untargeted bridged call must keep working untouched — the axis is additive"
    );
}

/// A well-formed `node` on an `<ext>.<tool>` call is accepted by the bridge and threaded to the
/// routed path — NOT rejected as a bad body, and NOT silently answered by the local node. Targeting
/// an absent node yields the honest refusal rather than a fabricated success, which is what proves
/// the field actually reached the routed path instead of being parsed and dropped.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_well_formed_node_is_threaded_to_the_routed_path_not_ignored() {
    let (gw, key) = gateway().await;
    let tok = token(&key, "user:a", "ws-a", &["mcp:demo.ping:call"]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post(
                "/mcp/call",
                call_body("demo.ping", Some("node:gw-not-here")),
            ),
            &tok,
        ))
        .await
        .unwrap();

    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a call targeted at an absent node must NOT return a success — a 200 here means `node` was \
         parsed and then dropped, i.e. the call silently ran untargeted"
    );
    assert_ne!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a well-formed node id on an <ext>.<tool> call must be accepted, not rejected as a bad body"
    );
}

/// A host-native verb (`store.*`, `undo`, `telemetry.*`, …) runs against THIS node's store — the
/// routed path only ever addresses an `<ext>.<tool>` queryable. So `node` on one of them must be
/// REFUSED (`400`), never ignored: ignoring it is the silent-fallback bug in its worst form — a
/// caller asks for `store.write` on gw-01, it executes locally, and the reply is an indistinguishable
/// `200`. This asserts the refusal is a shape error, not a denial (`403`) or an unreachable (`503`).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_targeted_host_native_verb_is_refused_rather_than_run_locally() {
    let (gw, key) = gateway().await;
    // Holding the capability, so a refusal here can only be about the call's shape.
    let tok = token(&key, "user:a", "ws-a", &["mcp:store.query:call"]);

    let resp = router(gw)
        .oneshot(bearer(
            json_post("/mcp/call", call_body("store.query", Some("node:gw-01"))),
            &tok,
        ))
        .await
        .unwrap();

    assert_eq!(
        resp.status(),
        StatusCode::BAD_REQUEST,
        "a targeted host-native verb must be refused as a malformed call — running it locally while \
         reporting success is the silent-fallback bug this scope exists to kill"
    );
}

/// Capability deny-test (mandatory). Addressing is not authorization: naming a node cannot widen
/// authority, and authorization runs BEFORE the node is looked at. So a capless caller gets the same
/// opaque `403` whether the node it named is plausible or invented — a targeted call is not an
/// oracle for discovering which nodes exist. Asserts it is never `409/503`, which would leak that.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_capless_targeted_call_is_denied_with_no_node_existence_signal() {
    let (gw, key) = gateway().await;
    // A real, authenticated principal holding an UNRELATED capability — so this is a capability
    // denial, not an authentication failure.
    let tok = token(&key, "user:a", "ws-a", &["mcp:something.else:call"]);

    // An `<ext>.<tool>` verb, so this exercises the ROUTED path — where the authorize-before-resolve
    // ordering is what stops a targeted call from becoming a fleet-enumeration oracle. (A host-native
    // verb would short-circuit on the shape check instead, which is a different guarantee.)
    for node in [None, Some("node:gw-01"), Some("node:invented-nowhere")] {
        let resp = router(gw.clone())
            .oneshot(bearer(
                json_post("/mcp/call", call_body("demo.ping", node)),
                &tok,
            ))
            .await
            .unwrap();

        assert_eq!(
            resp.status(),
            StatusCode::FORBIDDEN,
            "a capless call must be 403 regardless of the node named ({node:?}) — identical for a \
             real and an invented node, so it reveals nothing about the fleet"
        );
    }
}
