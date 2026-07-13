//! `media.serve` — read media bytes (original or variant) for the serve route (media scope).
//! Workspace + capability checked. The gateway's `GET /media/{id}` route calls this.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;

use super::error::MediaError;
use super::model::{media_get_raw, read_all_bytes, variant_read, MediaStatus};

/// The result of a serve read: the bytes + content type.
#[derive(Debug)]
pub struct ServedMedia {
    pub bytes: Vec<u8>,
    pub mime: String,
    pub etag: String,
}

/// Read media bytes for serving. `variant` is `None` for the original, or `Some("thumb")` for a
/// variant. Capability gate: `store:media/{id}:read`. Workspace-first (the store enforces it).
pub async fn media_serve(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    variant: Option<&str>,
) -> Result<ServedMedia, MediaError> {
    // Capability gate: store:media/{id}:read
    let req = Request::new(ws, Surface::Store, &format!("media/{id}"), Action::Read);
    if matches!(check(principal, &req), Decision::Denied(_)) {
        return Err(MediaError::Denied);
    }

    let media = media_get_raw(store, ws, id)
        .await?
        .ok_or(MediaError::NotFound)?;

    if media.status != MediaStatus::Ready {
        return Err(MediaError::NotReady);
    }

    let etag = format!("\"{}\"", &media.checksum);

    if let Some(vname) = variant {
        // Serve a variant.
        let variant_bytes = variant_read(store, ws, id, vname)
            .await?
            .ok_or(MediaError::NotFound)?;
        let mime = media
            .variants
            .iter()
            .find(|v| v.name == vname)
            .map(|v| v.mime.clone())
            .unwrap_or_else(|| media.mime.clone());
        Ok(ServedMedia {
            bytes: variant_bytes,
            mime,
            etag: format!("\"{}-{vname}\"", &media.checksum),
        })
    } else {
        // Serve the original.
        let bytes = read_all_bytes(store, ws, &media).await?;
        Ok(ServedMedia {
            bytes,
            mime: media.mime.clone(),
            etag,
        })
    }
}
