//! Discovery's error surface.
//!
//! Discovery is **best-effort and non-fatal by contract**: a node whose LAN filters mDNS must
//! still boot and serve. Callers are expected to log these and continue, never to abort boot —
//! see `advertise`'s module docs.

use thiserror::Error;

/// Why a discovery operation failed.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// The configured service type is not a valid DNS-SD type. A boot-time config error.
    #[error("invalid service type {ty:?}: {why}")]
    ServiceType { ty: String, why: &'static str },

    /// The mDNS daemon could not be started or a registration was refused — typically no
    /// multicast-capable interface, or the port is unavailable in a restricted container.
    #[error("mDNS responder error: {0}")]
    Responder(String),

    /// The advertised endpoint could not be parsed as `host:port`.
    #[error("invalid endpoint {endpoint:?}: {why}")]
    Endpoint { endpoint: String, why: &'static str },
}
