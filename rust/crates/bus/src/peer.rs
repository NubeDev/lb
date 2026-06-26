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
}

/// An embedded Zenoh peer. Cloneable handle to the live session.
#[derive(Clone)]
pub struct Bus {
    session: Session,
}

impl Bus {
    /// Open a default in-process peer session (solo node, S1).
    pub async fn peer() -> Result<Self, BusError> {
        let session = zenoh::open(zenoh::Config::default())
            .await
            .map_err(|e| BusError::Session(e.to_string()))?;
        Ok(Self { session })
    }

    /// The underlying session, for the pub/sub verbs added in S2.
    pub fn session(&self) -> &Session {
        &self.session
    }
}
