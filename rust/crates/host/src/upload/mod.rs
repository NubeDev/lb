//! `upload_sinks` — resumable bytes with no buffer (node-update scope §Seam 2).
//!
//! The trait lives here, in core host, because it is the vocabulary; the HTTP framing that drives it
//! lives in the gateway role (`routes/uploads/`), because bytes over HTTP are a transport concern.
//! Deliberately **not** MCP: binary bytes do not belong in a JSON tool call, which is precisely the
//! mistake `POST /extensions` is living with.

mod sink;

pub use sink::{
    UploadError, UploadHandle, UploadMeta, UploadSink, DEFAULT_MAX_UPLOAD_BYTES, UPLOAD_CHUNK_BYTES,
};
