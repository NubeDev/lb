//! The generic, resumable, **never-buffered** upload lane (node-update scope §Seam 2).
//!
//! ```text
//! POST   /uploads/{sink}                {size, digest_hex?, meta{…}}   → 201 {id, offset}
//! PATCH  /uploads/{sink}/{id}           Content-Range: bytes a-b/total → 200 {offset}
//! GET    /uploads/{sink}/{id}                                          → {id, offset}
//! POST   /uploads/{sink}/{id}/complete                                 → the sink's verdict
//! DELETE /uploads/{sink}/{id}                                          → abort
//! ```
//!
//! Mounted only when the embedder registered at least one sink; with none, the routes are absent and
//! every existing route is byte-for-byte unchanged. `{sink}` is an OPAQUE registry key — the core
//! names no sink (rule 10), and the same argument the outbox-target registry already settled applies
//! here: the embedder knows its backend, lb knows framing, bounds and the wall.

mod append;
mod begin;
mod content_range;
mod finish;
mod gate;
mod status;

pub use append::append_upload;
pub use begin::begin_upload;
pub use finish::{abort_upload, complete_upload};
pub use status::upload_status;
