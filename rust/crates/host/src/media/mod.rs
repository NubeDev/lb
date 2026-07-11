//! The **media** service — resumable chunked upload, variant derivation, and capability-checked
//! streaming serve (media scope). One datastore (SurrealDB — rule 2); bytes stored as chunk
//! records. The upload protocol (begin → chunk → commit) survives flaky cellular; variants
//! (thumb/preview) are derived on commit; the serve route checks workspace + capability + ETag.
//!
//! Verbs (one concern per file): `begin` / `chunk` / `commit` / `get` / `list` / `delete` /
//! `serve` / `range` / `variant` / `tool`. The chunk upload (`PUT /media/{id}/chunk/{n}`) and serve
//! (`GET /media/{id}`) are HTTP routes (bytes over HTTP, not MCP payloads).

mod begin;
mod chunk;
mod commit;
mod error;
mod get;
mod model;
mod range;
mod serve;
mod tool;
mod variant;

pub use begin::media_upload_begin;
pub use chunk::media_chunk_put;
pub use commit::media_upload_commit;
pub use error::MediaError;
pub use get::{media_delete, media_get, media_list};
pub use model::{chunk_write, MediaStatus, CHUNK_SIZE, CHUNK_TABLE};
pub use range::{plan_serve, ServePlan};
pub use serve::{media_serve, ServedMedia};
pub use tool::call_media_tool;
