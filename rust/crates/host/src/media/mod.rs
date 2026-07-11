//! The **media** service — resumable chunked upload, variant derivation, and capability-checked
//! streaming serve (media scope). One datastore (SurrealDB — rule 2); bytes stored as chunk
//! records. The upload protocol (begin → chunk → commit) survives flaky cellular; variants
//! (thumb/preview) are derived on commit; the serve route checks workspace + capability + ETag.
//!
//! Verbs (one concern per file): `begin` / `commit` / `get` / `list` / `delete` / `serve` /
//! `variant` / `tool`. The chunk upload (`PUT /media/{id}/chunk/{n}`) and serve
//! (`GET /media/{id}`) are HTTP routes (bytes over HTTP, not MCP payloads).

mod begin;
mod commit;
mod error;
mod get;
mod model;
mod serve;
mod tool;
mod variant;

pub use begin::{max_bytes_for_mime, media_upload_begin};
pub use commit::media_upload_commit;
pub use error::MediaError;
pub use get::{media_delete, media_get, media_list};
pub use model::{
    chunk_count, chunk_read, chunk_write, media_get_raw, media_list_raw, media_write, variant_read,
    variant_write, Media, MediaStatus, MediaVariant, VariantStatus, CHUNK_SIZE, CHUNK_TABLE,
    MEDIA_KIND, MEDIA_TABLE,
};
pub use serve::{media_meta, media_serve, ServedMedia};
pub use tool::call_media_tool;
pub use variant::derive_thumb;
