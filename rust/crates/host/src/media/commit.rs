//! `media.upload_commit` — verify all chunks + checksum, flip to `Ready`, enqueue variant job
//! (media scope). Gated by `mcp:media.upload:call`.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::error::MediaError;
use super::model::{
    media_get_raw, media_write, read_all_bytes, MediaStatus, MediaVariant, VariantStatus,
};

/// Commit an upload: verify all chunks present + checksum matches, flip to Ready, enqueue the
/// variant job. Returns `{ ok: true, variants: [...] }`.
pub async fn media_upload_commit(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    now: u64,
) -> Result<serde_json::Value, MediaError> {
    authorize_tool(principal, ws, "media.upload").map_err(|_| MediaError::Denied)?;

    let mut media = media_get_raw(store, ws, id)
        .await?
        .ok_or(MediaError::NotFound)?;

    if media.status != MediaStatus::Uploading {
        return Err(MediaError::BadInput(
            "media is not in uploading state".into(),
        ));
    }

    // Verify all chunks present + checksum.
    let bytes = read_all_bytes(store, ws, &media).await?;
    let computed = hex_sha256(&bytes);
    if computed != media.checksum {
        return Err(MediaError::BadChecksum);
    }

    // Flip to Ready.
    media.status = MediaStatus::Ready;
    media.ready_ts = now;

    // For images, enqueue a thumbnail variant derivation.
    if media.mime.starts_with("image/") {
        media.variants.push(MediaVariant {
            name: "thumb".into(),
            mime: "image/jpeg".into(),
            bytes: 0,
            status: VariantStatus::Pending,
        });
    }

    media_write(store, ws, &media).await?;

    // Derive variants synchronously (v1 — the job substrate exists but a thumbnail is fast enough
    // to derive inline; a heavy transcode would be a real job). A failure here leaves the media
    // Ready (the original is the authoritative copy); the variant is marked Failed.
    if media.mime.starts_with("image/") {
        let _ = super::variant::derive_thumb(store, ws, &media, &bytes).await;
    }

    Ok(json!({ "ok": true, "variants": media.variants }))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for b in hash {
        out.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        out.push(char::from_digit((b & 0xf) as u32, 16).unwrap());
    }
    out
}
