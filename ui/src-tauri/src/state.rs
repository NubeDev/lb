//! The shell's node state: a booted in-process node plus the session principal the UI acts
//! as. The shell IS a node (symmetric nodes, §3.1) — the window just attaches to it.
//!
//! S2 mints a demo member principal with channel grants so the UI can post/read. At S3 this
//! is replaced by a real verified session (login → token → principal). Kept in one place so
//! the commands stay thin.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::Node;

/// The live node + the principal the UI acts as. Held by the Tauri app as managed state.
pub struct NodeHandle {
    pub node: Node,
    pub principal: Principal,
    pub ws: String,
}

impl NodeHandle {
    /// Boot a solo node and mint the S2 demo session: a member in `ws` allowed to pub/sub the
    /// given channels (here, broad `bus:chan/*` grants for the demo).
    pub async fn boot(ws: &str) -> Result<Self, String> {
        let node = Node::boot().await.map_err(|e| e.to_string())?;
        let key = SigningKey::generate();
        let claims = Claims {
            sub: "user:me".into(),
            ws: ws.into(),
            role: Role::Member,
            caps: vec!["bus:chan/*:pub".into(), "bus:chan/*:sub".into()],
            iat: 0,
            exp: u64::MAX,
        };
        let token = mint(&key, &claims);
        let principal = verify(&key, &token, 1).map_err(|e| e.to_string())?;
        Ok(Self {
            node,
            principal,
            ws: ws.into(),
        })
    }
}
