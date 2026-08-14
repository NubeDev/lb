//! The **media** service — resumable chunked upload, variant derivation, and capability-checked
//! streaming serve (media scope). One datastore (SurrealDB — rule 2); bytes stored as chunk
//! records. The upload protocol (begin → chunk → commit) survives flaky cellular; variants
//! (thumb/preview) are derived on commit; the serve route checks workspace + capability + ETag.
//!
//! Verbs (one concern per file): `begin` / `chunk` / `commit` / `get` / `list` / `delete` /
//! `read` / `serve` / `range` / `variant` / `tool`. The chunk upload (`PUT /media/{id}/chunk/{n}`)
//! and serve (`GET /media/{id}`) are HTTP routes (bytes over HTTP, not MCP payloads) — that is the
//! primary path for anything that can set an `Authorization` header.
//!
//! `read` is the exception, and a narrow one: base64 bytes in bounded slices over the MCP bridge,
//! for callers that CANNOT set that header. A module-federated extension UI is the motivating case
//! — the host mounts it without the session token on purpose, so the HTTP route is a 401 for it and
//! the only alternative was lifting the token out of the host's `localStorage`. See `read.rs`.

mod begin;
mod chunk;
mod commit;
mod error;
mod get;
// `pub(crate)` so the extraction service (`assets/extract/`) can read media metadata + bytes
// (`media_get_raw`, `read_all_bytes`, `Media`) — extraction's source-of-truth input. Reads only;
// the write path (`begin`/`chunk`/`commit`) stays private to the media verbs.
pub(crate) mod model;
mod range;
mod read;
mod serve;
mod tool;
mod variant;

pub use begin::media_upload_begin;
pub use chunk::media_chunk_put;
pub use commit::media_upload_commit;
pub use error::MediaError;
pub use get::{media_delete, media_get, media_list};
pub use read::{media_read, MAX_READ_BYTES};
pub use model::{chunk_write, MediaStatus, CHUNK_SIZE, CHUNK_TABLE};
pub use range::{plan_serve, ServePlan};
pub use serve::{media_serve, ServedMedia};
pub use tool::call_media_tool;
