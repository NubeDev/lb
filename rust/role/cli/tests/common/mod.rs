//! Shared fixtures for the CLI integration tests: spin up a REAL gateway on a real socket, mint real
//! tokens, and seed real records through the real write path (no mocks, CLAUDE §9 / testing §0). A
//! `tests/common/` module is the standard Rust idiom for sharing helpers across integration binaries;
//! it is not itself a test binary. `dead_code` is allowed because each test file uses a subset.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::Arc;

use lb_auth::{mint, Claims, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};
use lb_role_gateway::{router, Gateway};

/// The fixed clock the gateway/tokens use (matches the gateway tests' `NOW`).
pub const NOW: u64 = 1000;

/// A running gateway: the base URL to POST to, the node behind it (to seed), and the signing key (to
/// mint tokens the gateway will accept). The `_shutdown` guard aborts the serve task on drop.
pub struct RunningGateway {
    pub base_url: String,
    pub node: Arc<Node>,
    pub key: SigningKey,
    _shutdown: tokio::task::JoinHandle<()>,
}

/// Boot a real Hub-role node, front it with a gateway on an ephemeral loopback port, and serve it in a
/// background task. Returns once the socket is bound and accepting — the `Remote` transport can hit it
/// immediately.
pub async fn spawn_gateway() -> RunningGateway {
    let node = Arc::new(Node::boot_as(NodeRole::Hub).await.expect("node boots"));
    let key = SigningKey::generate();
    let gw = Gateway::new(Arc::clone(&node), key.clone(), NOW);

    let listener = tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let app = router(gw);
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    RunningGateway {
        base_url: format!("http://{addr}"),
        node,
        key,
        _shutdown: handle,
    }
}

impl Drop for RunningGateway {
    fn drop(&mut self) {
        self._shutdown.abort();
    }
}

/// Mint a token signed by `key` for `(sub, ws, caps)`, valid at `NOW` (the gateway verifies with the
/// same key + clock).
pub fn token(key: &SigningKey, sub: &str, ws: &str, caps: &[&str]) -> String {
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: NOW - 1,
        exp: NOW + 10_000,
        constraint: None,
        run_id: None,
    };
    mint(key, &claims)
}

/// A member token carrying the FULL dev-login caps (parity with `/login`) for `(sub, ws)` — used when
/// a test needs a normally-authorized session (e.g. to read a seeded inbox).
pub fn dev_token(key: &SigningKey, sub: &str, ws: &str) -> String {
    let claims = lb_role_gateway::dev_claims(sub, ws, NOW, 10_000);
    mint(key, &claims)
}

/// Seed one inbox item through the REAL host write path (`record_inbox`), as a principal that holds
/// `inbox.record` in `ws`. The author is forced to the principal's sub host-side. This is a DB seed via
/// the real verb, not a mocked response.
pub async fn seed_inbox_item(node: &Node, ws: &str, channel: &str, id: &str, body: &str) {
    // A minimal principal holding exactly the record cap in `ws` (routed = the in-process co-trust path
    // the tests use to construct a real principal without a full login round-trip).
    let principal =
        lb_auth::Principal::routed("user:seed", ws, vec!["mcp:inbox.record:call".to_string()]);
    lb_host::record_inbox(&node.store, &principal, ws, channel, id, body, NOW)
        .await
        .expect("seed inbox item via the real write path");
}

/// Seed one reminder through the REAL host write path (`reminder_create`), as a principal holding
/// exactly `mcp:reminder.create:call` in `ws`. A channel-post action with a daily-09:00 schedule —
/// enough for list/get/delete tests. This is a DB seed via the real verb, not a mocked response.
pub async fn seed_reminder(node: &Node, ws: &str, id: &str, channel: &str, body: &str) {
    let principal = lb_auth::Principal::routed(
        "user:seed",
        ws,
        vec!["mcp:reminder.create:call".to_string()],
    );
    let action = lb_host::ReminderAction::ChannelPost {
        channel: channel.to_string(),
        body: body.to_string(),
    };
    lb_host::reminder_create(
        &node.store,
        &principal,
        ws,
        id,
        "0 9 * * *",
        None,
        action,
        NOW,
    )
    .await
    .expect("seed reminder via the real write path");
}
