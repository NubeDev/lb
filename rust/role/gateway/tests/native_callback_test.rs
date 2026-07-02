//! Native-sidecar → host MCP callback transport, end to end over REAL HTTP (native-callback-transport
//! scope). Proves the gap the native-tier scope deferred is closed: an out-of-process native sidecar,
//! carrying its supervisor-injected scoped token, can CALL host MCP tools through `POST /mcp/call` —
//! the same path an edge user uses (rule 7) — and is denied by the SAME capability + workspace gate.
//!
//! No mocks (CLAUDE §9 / testing §0): a real `Node`, a real `axum` gateway bound to a real TCP port,
//! the real `lb-sidecar-client` making real `reqwest` calls, the real MCP gate, a real embedded store.
//! The child token is minted with the NODE's signing key exactly as `native/spec.rs` does — so this
//! also proves the identity fix (a child token the gateway can VERIFY, not the old throwaway key).
//!
//! Mandatory categories:
//!   - happy round-trip: a granted callback reaches `series.find` and gets the real result;
//!   - capability-deny: a token WITHOUT `mcp:series.find:call` is refused (`CallError::Denied`), and
//!     the real handler is never reached (an isolation-seeded series is NOT leaked by the denial);
//!   - workspace-isolation: a ws-B token calling `series.find` via the callback sees NONE of ws-A's
//!     seeded series (the workspace is the token's, un-spoofable — the hard wall, §7).

use std::net::SocketAddr;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{
    ingest_write, tags_add, Facet, Node, Provenance, Qos, Role as NodeRole, Sample, Tag, TagSource,
};
use lb_role_gateway::{router, Gateway};
use lb_sidecar_client::{CallError, SidecarClient};
use serde_json::json;

const NOW: u64 = 1000;

/// Mint a child token the way `native/spec.rs` does: `sub = ext:<id>`, Member, the granted caps,
/// signed with the node's key so the gateway verifies it. `iat`/`exp` sit around `NOW`.
fn child_token(key: &SigningKey, ws: &str, ext_id: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: format!("ext:{ext_id}"),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
    };
    mint(key, &claims)
}

/// Boot a real node + a real gateway with a known key, serve it on a real ephemeral TCP port, and
/// return `(node, key, base_url)`. The node's key is the gateway's key (installed by `Gateway::new`),
/// so a token minted with `key` verifies on the callback.
async fn serve() -> (Arc<Node>, SigningKey, String) {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(node.clone(), key.clone(), NOW);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr: SocketAddr = listener.local_addr().unwrap();
    let app = router(gw);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (node, key, format!("http://{addr}"))
}

/// Seed a real, findable series in `ws`: write one labeled sample and drain so the tag facets commit.
/// After this, `series.find` with the matching facet returns the series name in `ws` (and only `ws`).
async fn seed_series(node: &Node, key: &SigningKey, ws: &str, series: &str, fleet: &str) {
    let principal = lb_auth::verify(
        key,
        &child_token(
            key,
            ws,
            "seeder",
            &["mcp:ingest.write:call", "mcp:tags.add:call"],
        ),
        NOW,
    )
    .expect("seeder verifies");
    let sample = Sample {
        series: series.into(),
        producer: "ext:seeder".into(),
        ts: 1,
        seq: 1,
        payload: json!(21),
        labels: json!({}),
        qos: Qos::BestEffort,
    };
    let n = ingest_write(&node.store, &principal, ws, vec![sample])
        .await
        .expect("write sample");
    assert_eq!(n, 1, "one sample staged");
    // `ingest_write` only STAGES; drain the workspace so the sample commits.
    lb_host::drain_workspace(&node.store, ws)
        .await
        .expect("drain commits the staged sample");
    // Discovery via `series.find` is over the TAG graph (tags scope), not sample labels: assert a real
    // tag edge on the series entity so a matching facet finds it — the way a producer tags a series.
    tags_add(
        &node.store,
        &principal,
        ws,
        series,
        &Tag::new("fleet", json!(fleet)),
        &Provenance::new(NOW, principal.sub().to_string(), TagSource::Producer),
    )
    .await
    .expect("tag the series so series.find can discover it");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn granted_sidecar_callback_reaches_series_find() {
    let (node, key, base) = serve().await;
    let ws = "cb-happy";
    seed_series(&node, &key, ws, "series:fleet.temp", "alpha").await;

    // A child granted `mcp:series.find:call`, exactly what fleet-monitor's manifest requests.
    let token = child_token(&key, ws, "fleet-monitor", &["mcp:series.find:call"]);
    let client = SidecarClient::with_config(lb_sidecar_client::Config::new(
        &base,
        token,
        ws,
        "fleet-monitor",
    ));

    // Find by the seeded facet — the real callback round-trips and returns the real series name.
    let out = client
        .call_tool(
            "series.find",
            json!({ "facets": [{ "key": "fleet", "value": "alpha" }] }),
        )
        .await
        .expect("granted callback succeeds");
    let hits = out
        .get("series")
        .and_then(|v| v.as_array())
        .expect("series array");
    assert!(
        hits.iter().any(|s| s == "series:fleet.temp"),
        "the granted callback saw the seeded series, got {out}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ungranted_sidecar_callback_is_denied_and_leaks_nothing() {
    let (node, key, base) = serve().await;
    let ws = "cb-deny";
    // Seed a series so a leak would be observable if the gate were bypassed.
    seed_series(&node, &key, ws, "series:secret.temp", "beta").await;

    // A child token WITHOUT `mcp:series.find:call` (it holds an unrelated cap) — the gate must refuse.
    let token = child_token(&key, ws, "fleet-monitor", &["mcp:series.latest:call"]);
    let client = SidecarClient::with_config(lb_sidecar_client::Config::new(
        &base,
        token,
        ws,
        "fleet-monitor",
    ));

    let err = client
        .call_tool(
            "series.find",
            json!({ "facets": [{ "key": "fleet", "value": "beta" }] }),
        )
        .await
        .expect_err("ungranted callback must be refused");
    // The refusal is the distinct capability-deny variant (a `403`) — never conflated with transport,
    // and it carries no series data (the real handler was never reached).
    assert!(
        matches!(err, CallError::Denied),
        "expected CallError::Denied, got {err:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn ws_b_callback_cannot_see_ws_a_series() {
    let (node, key, base) = serve().await;
    let ws_a = "cb-iso-a";
    let ws_b = "cb-iso-b";
    // ws-A has a series under the `fleet=gamma` facet; ws-B does not.
    seed_series(&node, &key, ws_a, "series:a.temp", "gamma").await;

    // A ws-B child, granted `series.find` IN ws-B — the grant is real, the workspace is the wall.
    let token = child_token(&key, ws_b, "fleet-monitor", &["mcp:series.find:call"]);
    let client = SidecarClient::with_config(lb_sidecar_client::Config::new(
        &base,
        token,
        ws_b,
        "fleet-monitor",
    ));

    // The SAME facet ws-A's series carries — ws-B must see none of it (the workspace comes from the
    // token, never the body; a ws-B caller can't reach ws-A data through the callback).
    let out = client
        .call_tool(
            "series.find",
            json!({ "facets": [{ "key": "fleet", "value": "gamma" }] }),
        )
        .await
        .expect("ws-b callback is authorized in its own ws");
    let hits = out
        .get("series")
        .and_then(|v| v.as_array())
        .expect("series array");
    assert!(
        hits.is_empty(),
        "ws-B must not see ws-A's series through the callback, got {out}"
    );

    // Sanity: the series really does exist in ws-A (so the empty ws-B result is isolation, not a
    // seeding failure) — a direct in-process find under a ws-A principal returns it.
    let principal_a = lb_auth::verify(
        &key,
        &child_token(&key, ws_a, "checker", &["mcp:series.find:call"]),
        NOW,
    )
    .expect("ws-a checker verifies");
    let found_a = lb_host::series_find(
        &node.store,
        &principal_a,
        ws_a,
        &[Facet::exact("fleet", json!("gamma"))],
    )
    .await
    .expect("ws-a find");
    assert!(
        found_a.iter().any(|s| s == "series:a.temp"),
        "ws-A really has the series (isolation, not a seed miss)"
    );
}
