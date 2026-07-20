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
    /// The **workspace** this request arrived for, recovered from the concrete key it landed on
    /// (`ws/{ws}/…`). A queryable may be declared with a `*` workspace wildcard (one declaration
    /// serves every workspace — see `serve_ext`), but each individual `get` targets a CONCRETE key,
    /// so the `{ws}` segment is always present here. The serving side needs it to resolve a
    /// per-workspace target (a native sidecar keyed `(ws, ext_id)`); `None` if the key is malformed.
    pub fn ws(&self) -> Option<String> {
        // key = `ws/{ws}/{rel}` — the workspace is the second `/`-segment.
        self.query
            .key_expr()
            .as_str()
            .split('/')
            .nth(1)
            .map(String::from)
    }

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

/// Send `request` to the queryable on `(ws, rel)` and await the reply bytes. `None` if no node
/// answered (e.g. the hosting node is offline) — the caller maps that to its own error.
///
/// **Enforces "exactly one responder" rather than assuming it** (routed-node-dispatch, #81). This
/// used to take the first successful reply and say so in a comment; that comment WAS the bug — with
/// two nodes answering one key, the caller silently kept whichever won the race and had no way to
/// know a second existed. Now a second responder is a [`BusError::MultipleResponders`].
///
/// The check is deliberately at the CALL SITE, not the serving side: a serving node knows only
/// about itself, so detecting a duplicate there would require cross-node coordination. Here, the
/// replies all arrive in one place and the duplicate is simply visible.
///
/// **Cost.** After the first reply we drain the remaining replies, which is why this does not slow
/// the common case: Zenoh closes the reply channel once every matching queryable has answered (or
/// the query completes), so with one responder `recv_async` returns `Err` immediately and there is
/// no added wait — no extra hop, no timer, no latency on the single-host path this scope promised
/// to leave unchanged.
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

    let mut first: Option<Vec<u8>> = None;
    while let Ok(reply) = replies.recv_async().await {
        if let Ok(sample) = reply.result() {
            let bytes = sample.payload().to_bytes().to_vec();
            if first.is_some() {
                // A second node answered a key that only one node should declare. Refusing is the
                // point: silently keeping one of two answers is how a caller ends up acting on the
                // wrong node's result.
                return Err(BusError::MultipleResponders { key });
            }
            first = Some(bytes);
        }
    }
    Ok(first)
}
