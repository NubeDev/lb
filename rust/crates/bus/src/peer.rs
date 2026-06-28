//! Open and hold an embedded Zenoh peer session. The host owns one of these for its lifetime.
//!
//! S1 opens a default peer (solo). The peer/router mode and upstream endpoints are config
//! (symmetric nodes) — wired in when the multi-node slice lands (S3). Pub/sub verbs arrive
//! with the messaging slice (S2); here we prove the peer boots in-process.

use thiserror::Error;
use zenoh::Session;

#[derive(Debug, Error)]
pub enum BusError {
    #[error("bus session error: {0}")]
    Session(String),
    #[error("bus config error: {0}")]
    Config(String),
}

/// An embedded Zenoh peer. Cloneable handle to the live session.
#[derive(Clone)]
pub struct Bus {
    session: Session,
}

impl Bus {
    /// Open a default in-process peer session (solo node, S1).
    ///
    /// Every node — edge, hub, or solo — opens a Zenoh **peer** here; the in-process peers
    /// auto-discover and form one network (the multi-node substrate, S3). Whether a node also
    /// runs a router or connects to an upstream endpoint is *config* set by the role/deployment
    /// layer (README §3.1), never a branch in this crate. S3 proves the second node with two
    /// peers on the same network; explicit endpoint config is a deployment concern (S7).
    pub async fn peer() -> Result<Self, BusError> {
        let session = zenoh::open(zenoh::Config::default())
            .await
            .map_err(|e| BusError::Session(e.to_string()))?;
        Ok(Self { session })
    }

    /// Open a peer with explicit **listen** and/or **connect** endpoints — the same posture the
    /// deployment layer wires for real multi-node (README §3.1, §6.2: the peer/router mode and
    /// upstream endpoints are *config*, never a code branch). Each endpoint is a Zenoh locator
    /// string, e.g. `"tcp/127.0.0.1:0"` (OS-assigned port) to listen, or a concrete
    /// `"tcp/127.0.0.1:43187"` to connect to.
    ///
    /// Why this exists beyond `peer()`: ambient multicast scouting is how solo/dev peers find each
    /// other (`peer()`), but it is **best-effort and non-deterministic** — under many concurrent
    /// in-process peers (e.g. a full parallel `cargo test --workspace`, hundreds of peers in one
    /// multicast scout domain) gossip between a *specific* pair can stall indefinitely. An explicit
    /// endpoint gives a deterministic point-to-point link, independent of the scout domain. This is
    /// the production-faithful path too: real edge↔hub links are configured endpoints, not luck.
    pub async fn peer_with(listen: &[String], connect: &[String]) -> Result<Self, BusError> {
        let mut config = zenoh::Config::default();
        if !listen.is_empty() {
            config
                .insert_json5("listen/endpoints", &json_strings(listen))
                .map_err(|e| BusError::Config(e.to_string()))?;
        }
        if !connect.is_empty() {
            config
                .insert_json5("connect/endpoints", &json_strings(connect))
                .map_err(|e| BusError::Config(e.to_string()))?;
        }
        let session = zenoh::open(config)
            .await
            .map_err(|e| BusError::Session(e.to_string()))?;
        Ok(Self { session })
    }

    /// The underlying session, for the pub/sub verbs added in S2.
    pub fn session(&self) -> &Session {
        &self.session
    }
}

/// Render a list of locator strings as a JSON5 array literal for `insert_json5`.
fn json_strings(items: &[String]) -> String {
    let quoted: Vec<String> = items
        .iter()
        .map(|s| format!("{:?}", s)) // debug-quotes + escapes the string safely
        .collect();
    format!("[{}]", quoted.join(","))
}
