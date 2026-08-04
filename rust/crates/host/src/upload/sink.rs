//! [`UploadSink`] — the embedder-registered destination for a resumable, **never-buffered** binary
//! upload (node-update scope §Seam 2).
//!
//! On an airgapped box the new artifact has to arrive somehow, and a data-federation sidecar is
//! measured in gigabytes. lb's only large-payload ingest today (`POST /extensions`) JSON-encodes the
//! artifact into a byte array at roughly 8× inflation and holds it in memory; nothing in that shape
//! survives a multi-GB artifact on a 959 MB edge box.
//!
//! **lb owns framing, resumption, bounds and the wall — never a byte of durable artifact state.**
//! Digest verification, signature trust and the content-addressed cache belong to the sink's
//! backend, which already has all three; a second trust chain in lb would be one more thing free to
//! disagree. The registry names nothing (rule 10) — a sink called `"package"` on one host and
//! `"firmware"` on another needs no lb change, and lb never learns whose bytes it moved.

use async_trait::async_trait;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The default per-sink ceiling when a sink does not declare one: 4 GiB. **Bounded, never unlimited**
/// — a sink that wants more says so, and a sink that says nothing still cannot be used to fill a
/// disk without end.
pub const DEFAULT_MAX_UPLOAD_BYTES: u64 = 4 * 1024 * 1024 * 1024;

/// The chunk size lb forwards to [`UploadSink::append`]. Peak memory for an upload of ANY size is one
/// of these, because the request body is read as a stream and handed over in bounded pieces — never
/// collected, never spooled to lb's own disk.
pub const UPLOAD_CHUNK_BYTES: usize = 64 * 1024;

/// What a client declares at `begin`, and what lb hands back verbatim at `complete`. `digest_hex`
/// and `meta` are **opaque** to lb: it carries them, it never interprets them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadMeta {
    /// The declared total size in bytes. Checked at `begin` against the sink's ceiling and enforced
    /// as a running total during append.
    pub size: u64,
    /// The artifact's content digest, when the client knows it. **This is the resume identity**: a
    /// `begin` carrying a digest the sink already holds a partial for returns the EXISTING handle
    /// and its offset, not a fresh id — so a client that lost its id (browser refresh, new session)
    /// resumes instead of double-filling the backend's disk with a second partial.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub digest_hex: Option<String>,
    /// Whatever else the sink's backend needs. Opaque to lb.
    #[serde(default)]
    pub meta: Value,
}

/// The sink's durable handle for one upload: its own id, and the offset it ALREADY holds (non-zero
/// when this upload is being resumed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadHandle {
    pub id: String,
    pub offset: u64,
}

/// A sink's typed refusal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UploadError {
    /// No such upload on this sink (or it was already aborted/completed).
    #[error("no such upload")]
    NotFound,
    /// The chunk did not begin at the sink's current offset. **Offsets are the sink's truth**, so the
    /// refusal carries the correct one and a client that lost track resumes without guessing.
    #[error("offset mismatch: expected {expected}")]
    Offset { expected: u64 },
    /// The declared size (at `begin`) or the running total (during `append`) exceeded the ceiling.
    #[error("too large: limit {limit} bytes")]
    TooLarge { limit: u64 },
    /// The sink refuses for its own stated reason — no disk, wrong digest shape, a failed
    /// verification at `complete`. Passed through with the reason; the sink MUST sanitize it.
    #[error("rejected: {0}")]
    Rejected(String),
    /// The sink's backend faulted.
    #[error("backend: {0}")]
    Backend(String),
}

/// An embedder-registered upload destination. lb calls it; it never calls lb.
#[async_trait]
pub trait UploadSink: Send + Sync {
    /// The capability a caller must hold to use this lane. **The sink chooses; lb enforces** — on
    /// EVERY call in the sequence, not only at `begin`, so a session that loses its grant mid-upload
    /// stops there.
    ///
    /// It must be a string in the platform's `surface:resource:action` capability grammar (e.g.
    /// `mcp:package.upload:call`) — the same grammar every other grant speaks, checked through the
    /// same wall. lb never interprets the string beyond that; an UNPARSEABLE cap is held by nobody,
    /// so a malformed declaration fails closed and the lane is simply unreachable.
    fn required_cap(&self) -> &str;

    /// This sink's ceiling in bytes. Checked at `begin` against the declared size and enforced as a
    /// running total during append. Defaults to [`DEFAULT_MAX_UPLOAD_BYTES`] — bounded, never
    /// unlimited.
    fn max_upload_bytes(&self) -> u64 {
        DEFAULT_MAX_UPLOAD_BYTES
    }

    /// Begin — the sink allocates its own durable id and reports the offset it already holds
    /// (non-zero when this upload is being resumed against the sink's backend, keyed by
    /// [`UploadMeta::digest_hex`]).
    async fn begin(&self, meta: &UploadMeta) -> Result<UploadHandle, UploadError>;

    /// The sink's current offset for `id` — what `GET /uploads/{sink}/{id}` reports, and how a
    /// client that lost track re-synchronises after a dropped link. A read, never a mutation.
    async fn status(&self, id: &str) -> Result<UploadHandle, UploadError>;

    /// Append one chunk at `offset`, returning the new offset. Called repeatedly; **must be
    /// idempotent per `(id, offset)`** so a retried chunk cannot double-write. A chunk that does not
    /// begin at the sink's current offset is [`UploadError::Offset`] carrying the correct one.
    async fn append(&self, id: &str, offset: u64, chunk: Bytes) -> Result<u64, UploadError>;

    /// Finalize — the sink verifies and commits. lb reports the sink's verdict **verbatim**.
    async fn complete(&self, id: &str, meta: &UploadMeta) -> Result<Value, UploadError>;

    /// Discard a partial upload.
    async fn abort(&self, id: &str) -> Result<(), UploadError>;
}
