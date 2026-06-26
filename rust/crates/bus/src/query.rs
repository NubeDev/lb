//! Request/response over the bus — a workspace-scoped Zenoh **queryable** (motion, §3.3).
//!
//! Pub/sub moves one-way messages; a queryable answers a *request*. This is the transport the
//! routed MCP tool call rides on (mcp scope): node A `query`s `ws/{id}/{rel}` and the node
//! hosting the extension answers from its `declare_queryable`. Like every bus key it is
//! workspace-scoped via `ws_key`, so a query in workspace B can never reach a queryable
//! declared in workspace A — the workspace wall is structural on the request path too (§7).
//!
//! The bus only carries opaque request/reply *bytes*; the caller (the mcp routing seam) owns
//! the payload shape. Durability is not here — a routed call is live motion; if the remote
//! node is offline the query simply finds no responder (the caller decides what that means).

use zenoh::handlers::FifoChannelHandler;
use zenoh::query::{Query, Queryable};

use crate::key::ws_key;
use crate::peer::{Bus, BusError};

/// A declared queryable: the host answers requests on `(ws, rel)` until this is dropped. Each
/// inbound request is awaited via [`Responder::recv`], then replied to with [`Responder::reply`].
pub struct Responder {
    inner: Queryable<FifoChannelHandler<Query>>,
    ws: String,
    rel: String,
}

/// One inbound request awaiting a reply. Carries the request bytes; reply once with [`reply`].
pub struct Incoming {
    query: Query,
}

impl Incoming {
    /// The request payload bytes (empty if the caller sent none).
    pub fn payload(&self) -> Vec<u8> {
        self.query
            .payload()
            .map(|p| p.to_bytes().to_vec())
            .unwrap_or_default()
    }

    /// Answer this request with `reply` bytes, on the same key it arrived on.
    pub async fn reply(self, reply: &[u8]) -> Result<(), BusError> {
        let key = self.query.key_expr().clone();
        self.query
            .reply(key, reply.to_vec())
            .await
            .map_err(|e| BusError::Session(e.to_string()))
    }
}

impl Responder {
    /// Await the next inbound request. `None` once the queryable is closed.
    pub async fn recv(&self) -> Option<Incoming> {
        let query = self.inner.recv_async().await.ok()?;
        Some(Incoming { query })
    }

    /// The workspace-scoped key this responder answers on (for logging/routing).
    pub fn key(&self) -> String {
        ws_key(&self.ws, &self.rel)
    }
}

/// Declare a queryable answering requests on `(ws, rel)`. The host serving an extension's tools
/// to remote callers holds one of these.
pub async fn declare_queryable(bus: &Bus, ws: &str, rel: &str) -> Result<Responder, BusError> {
    let key = ws_key(ws, rel);
    let inner = bus
        .session()
        .declare_queryable(&key)
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;
    Ok(Responder {
        inner,
        ws: ws.to_string(),
        rel: rel.to_string(),
    })
}

/// Send `request` to the queryable on `(ws, rel)` and await the first reply's bytes. `None` if
/// no node answered (e.g. the hosting node is offline) — the caller maps that to its own error.
pub async fn query(
    bus: &Bus,
    ws: &str,
    rel: &str,
    request: &[u8],
) -> Result<Option<Vec<u8>>, BusError> {
    let key = ws_key(ws, rel);
    let replies = bus
        .session()
        .get(&key)
        .payload(request.to_vec())
        .await
        .map_err(|e| BusError::Session(e.to_string()))?;

    // Take the first successful reply; a routed tool call has exactly one responder.
    while let Ok(reply) = replies.recv_async().await {
        if let Ok(sample) = reply.result() {
            return Ok(Some(sample.payload().to_bytes().to_vec()));
        }
    }
    Ok(None)
}
