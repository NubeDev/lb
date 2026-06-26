//! The gateway's shared state: the in-process node it fronts plus the session principal each
//! request acts as. The gateway IS a node (symmetric nodes, §3.1) — it just also exposes an
//! HTTP/SSE surface so a *browser* can reach it (README §6.13). It adds no authority of its own;
//! every route forwards to `lb_host::*` with this principal, so the SAME capability check guards
//! the browser as guards the desktop shell and every other caller (capability-first, §3.5).
//!
//! S3 mints a demo member principal (like the Tauri shell's `state.rs`). A real verified session
//! (login → token → principal) lands with auth wiring later; kept in one place so routes stay thin.

use std::sync::Arc;

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{Node, Role as NodeRole};

/// The live node + the principal the browser session acts as, shared across handlers (`Arc` so
/// axum can clone it into each request).
#[derive(Clone)]
pub struct Gateway {
    pub node: Arc<Node>,
    pub principal: Arc<Principal>,
    pub ws: String,
}

impl Gateway {
    /// Boot a gateway-role node and mint the demo session: a member in `ws` with broad channel
    /// grants (matching the Tauri shell's demo so the same UI works against either transport).
    pub async fn boot(ws: &str) -> Result<Self, String> {
        let node = Node::boot_as(NodeRole::Hub)
            .await
            .map_err(|e| e.to_string())?;
        let principal = demo_principal(ws)?;
        Ok(Self::with_principal(node, principal, ws))
    }

    /// Build a gateway around an existing node + an explicit principal/workspace. Lets a caller
    /// (and the tests) front a node with a specific session — e.g. a principal WITHOUT grants
    /// to prove the deny path, or one scoped to another workspace to prove isolation.
    pub fn with_principal(node: Node, principal: Principal, ws: &str) -> Self {
        Self::from_shared(Arc::new(node), principal, ws)
    }

    /// Build a gateway around a SHARED node (`Arc<Node>`) + a session. Two gateways over one node
    /// — e.g. two browser sessions in different workspaces — prove that the workspace wall holds
    /// at the gateway: each only ever sees its own workspace's data through the same store/bus.
    pub fn from_shared(node: Arc<Node>, principal: Principal, ws: &str) -> Self {
        Self {
            node,
            principal: Arc::new(principal),
            ws: ws.to_string(),
        }
    }
}

/// Mint + verify the demo member principal (broad `bus:chan/*` grants for the demo workspace).
fn demo_principal(ws: &str) -> Result<Principal, String> {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: "user:browser".into(),
        ws: ws.into(),
        role: Role::Member,
        caps: vec!["bus:chan/*:pub".into(), "bus:chan/*:sub".into()],
        iat: 0,
        exp: u64::MAX,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).map_err(|e| e.to_string())
}
