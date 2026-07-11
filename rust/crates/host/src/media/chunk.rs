//! `media.chunk_put` — validate + write one upload chunk (media scope). The gateway's
//! `PUT /media/{id}/chunk/{n}` route calls this; every check runs **before any byte is written**
//! (deny = 4xx before storage, per the scope). Gated by `mcp:media.upload:call` — the same cap
//! as `upload_begin`/`commit`, so an authenticated-but-uncapped caller cannot write chunks.
//!
//! Validation (in order): capability → upload exists → status is `Uploading` (a chunk re-PUT
//! after `Ready` would silently change served bytes while the ETag stayed stale) → `n` in range
//! → body no larger than the declared chunk size. Idempotent on success (re-PUT upserts).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MediaError;
use super::model::{chunk_write, media_get_raw, MediaStatus};

/// Validate and write one chunk of an in-flight upload.
pub async fn media_chunk_put(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    n: u32,
    bytes: &[u8],
) -> Result<(), MediaError> {
    authorize_tool(principal, ws, "media.upload").map_err(|_| MediaError::Denied)?;

    let media = media_get_raw(store, ws, id)
        .await?
        .ok_or(MediaError::NotFound)?;

    if media.status != MediaStatus::Uploading {
        return Err(MediaError::BadInput(
            "media is not in uploading state".into(),
        ));
    }
    if n >= media.chunks {
        return Err(MediaError::BadInput(format!(
            "chunk {n} out of range (upload declared {} chunks)",
            media.chunks
        )));
    }
    if bytes.len() as u64 > media.chunk_size as u64 {
        return Err(MediaError::TooLarge);
    }

    chunk_write(store, ws, id, n, bytes).await?;
    Ok(())
}
