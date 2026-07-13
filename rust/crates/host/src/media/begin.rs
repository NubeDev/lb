//! `media.upload_begin` — declare a new upload (media scope). Gated by `mcp:media.upload:call`.
//! Creates a `Media` record with status `Uploading`, computes the chunk count, returns the upload
//! id + chunk size. The caller PUTs chunks via `PUT /media/{id}/chunk/{n}`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::error::MediaError;
use super::model::{chunk_count, media_write, Media, CHUNK_SIZE};

/// The per-mime max size (v1). Images: 50 MiB; video: 500 MiB; other: 50 MiB.
pub fn max_bytes_for_mime(mime: &str) -> u64 {
    if mime.starts_with("video/") {
        500 * 1024 * 1024
    } else {
        50 * 1024 * 1024
    }
}

/// Begin a new upload. Returns `{ id, chunk_size, chunks }`.
pub async fn media_upload_begin(
    store: &Store,
    principal: &Principal,
    ws: &str,
    mime: &str,
    declared_bytes: u64,
    checksum: &str,
    origin: Option<&str>,
    now: u64,
) -> Result<serde_json::Value, MediaError> {
    authorize_tool(principal, ws, "media.upload").map_err(|_| MediaError::Denied)?;

    if mime.is_empty() {
        return Err(MediaError::BadInput("missing mime".into()));
    }
    let max = max_bytes_for_mime(mime);
    if declared_bytes > max {
        return Err(MediaError::TooLarge);
    }

    // Generate a media id (deterministic from the checksum + owner + timestamp).
    let id = media_id(principal, checksum, now);
    let chunks = chunk_count(declared_bytes, CHUNK_SIZE);
    let mut media = Media::new(
        &id,
        mime,
        declared_bytes,
        checksum,
        principal.sub(),
        chunks,
        CHUNK_SIZE,
        now,
    );
    media.origin = origin.map(|s| s.to_string());
    media_write(store, ws, &media).await?;

    Ok(json!({
        "id": id,
        "chunk_size": CHUNK_SIZE,
        "chunks": chunks,
    }))
}

fn media_id(principal: &Principal, checksum: &str, now: u64) -> String {
    let mut hasher = Sha256::new();
    hasher.update(principal.sub().as_bytes());
    hasher.update(checksum.as_bytes());
    hasher.update(now.to_le_bytes());
    let hash = hasher.finalize();
    // 12 hex chars = 48 bits — collision-resistant per workspace per upload session.
    hex(&hash[..6])
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}
