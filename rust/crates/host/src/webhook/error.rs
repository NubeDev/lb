//! The webhook service errors (webhooks scope). `Denied` is the opaque cap-deny / wrong-secret
//! surface; `NotFound` covers a disabled/unknown/tombstoned hook (the route collapses both to the
//! same opaque 404 so the public route doesn't become a webhook-id oracle). The remaining variants
//! distinguish the management-verb paths (bad input, a no-widening refusal, store failures).

use lb_store::StoreError;

/// Why a webhook verb failed. The inbound path collapses every auth failure to `NotFound` (no
/// existence leak); the management verbs surface `Denied`/`BadInput`/`Widen` for the admin console.
#[derive(thiserror::Error, Debug)]
pub enum WebhookError {
    /// A capability deny (`mcp:webhook.manage:call` or `mcp:ingest.write:call`).
    #[error("denied")]
    Denied,
    /// The webhook (or linked apikey / secret) does not exist, or is tombstoned. The route maps
    /// this to an opaque `404` so the public endpoint is not a webhook-id oracle.
    #[error("not found")]
    NotFound,
    /// The presented credential was missing/wrong (inbound path). The route collapses this to the
    /// same opaque `401` as `NotFound` — indistinguishable to the caller (no oracle).
    #[error("invalid credential")]
    Invalid,
    /// The webhook is revoked (tombstoned). The route maps this to `410 Gone` (no further hits).
    #[error("revoked")]
    Revoked,
    /// An admin verb was given malformed input (empty name, unknown auth_mode, …).
    #[error("{0}")]
    BadInput(String),
    /// `webhook.create` no-widening refusal: the creator lacks `mcp:ingest.write:call`, so minting
    /// a webhook whose principal would resolve to it would widen beyond the admin (the same guard
    /// `apikey.create` runs, applied to the hook's effective caps).
    #[error("cannot grant a cap the creator lacks: {0}")]
    Widen(String),
    /// A store error. Kept opaque at the boundary (the route maps it to a generic 5xx / opaque 4xx).
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<crate::ingest::IngestError> for WebhookError {
    /// The inbound accept path threads `ingest_write`/`drain_workspace` errors here. A denied
    /// ingest (the synthetic principal somehow lacked `mcp:ingest.write:call`) is `Denied`; a
    /// store failure stays a store failure; bad input is bad input.
    fn from(e: crate::ingest::IngestError) -> Self {
        use crate::ingest::IngestError;
        match e {
            IngestError::Denied => WebhookError::Denied,
            IngestError::BadInput(m) => WebhookError::BadInput(m),
            IngestError::Store(s) => WebhookError::Store(s),
        }
    }
}
