//! `media.get` / `media.list` / `media.delete` — the metadata CRUD (media scope).

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::MediaError;
use super::model::{media_get_raw, media_list_raw, media_write, Media, MediaStatus};

/// Get media metadata by id. Gated by `mcp:media.get:call`.
pub async fn media_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Media, MediaError> {
    authorize_tool(principal, ws, "media.get").map_err(|_| MediaError::Denied)?;
    media_get_raw(store, ws, id)
        .await?
        .ok_or(MediaError::NotFound)
}

/// List all media in the workspace. Gated by `mcp:media.get:call`.
pub async fn media_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Media>, MediaError> {
    authorize_tool(principal, ws, "media.get").map_err(|_| MediaError::Denied)?;
    Ok(media_list_raw(store, ws).await?)
}

/// Archive (soft-delete) media by id. Gated by `mcp:media.delete:call`.
pub async fn media_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), MediaError> {
    authorize_tool(principal, ws, "media.delete").map_err(|_| MediaError::Denied)?;
    let mut media = media_get_raw(store, ws, id)
        .await?
        .ok_or(MediaError::NotFound)?;
    media.status = MediaStatus::Archived;
    media_write(store, ws, &media).await?;
    Ok(())
}
