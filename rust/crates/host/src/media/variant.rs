//! Variant derivation — thumbnail generation for images (media scope). Uses the `image` crate to
//! decode + resize + encode. Runs in the commit path (v1 — fast enough for a thumbnail; a heavy
//! transcode would be a real job). A failure marks the variant `Failed` but leaves the media
//! `Ready` (the original is the authoritative copy).

use image::ImageReader;
use lb_store::Store;
use std::io::Cursor;

use super::error::MediaError;
use super::model::{media_get_raw, media_write, variant_write, MediaVariant, VariantStatus};

/// The thumbnail max dimension (pixels). The longest side is scaled to this; aspect preserved.
const THUMB_MAX: u32 = 256;

/// Derive a thumbnail for `media` from its raw `bytes`. Updates the variant status on the record.
pub async fn derive_thumb(
    store: &Store,
    ws: &str,
    media: &super::model::Media,
    bytes: &[u8],
) -> Result<(), MediaError> {
    // Decode the image (attacker-controlled input — the `image` crate is hardened; size caps
    // are enforced by the declared_bytes check at begin).
    let thumb_bytes = tokio::task::spawn_blocking({
        let bytes = bytes.to_vec();
        move || -> Result<Vec<u8>, String> {
            let reader = ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()
                .map_err(|e| e.to_string())?;
            let img = reader.decode().map_err(|e| e.to_string())?;
            let thumb = img.resize(THUMB_MAX, THUMB_MAX, image::imageops::FilterType::Lanczos3);
            let mut buf = Vec::new();
            let mut writer = Cursor::new(&mut buf);
            thumb
                .write_to(&mut writer, image::ImageFormat::Jpeg)
                .map_err(|e| e.to_string())?;
            Ok(buf)
        }
    })
    .await
    .map_err(|e| MediaError::Store(e.to_string()))?
    .map_err(|e| MediaError::Store(e))?;

    // Store the variant bytes.
    variant_write(store, ws, &media.id, "thumb", &thumb_bytes).await?;

    // Update the variant status on the record.
    let mut updated = media_get_raw(store, ws, &media.id)
        .await?
        .ok_or(MediaError::NotFound)?;
    for v in &mut updated.variants {
        if v.name == "thumb" {
            v.status = VariantStatus::Ready;
            v.bytes = thumb_bytes.len() as u64;
        }
    }
    media_write(store, ws, &updated).await?;
    Ok(())
}
