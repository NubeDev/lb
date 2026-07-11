//! The **media** model — the metadata record for a chunked-upload binary (media scope). The
//! content bytes live as chunk records in the same SurrealDB store (one datastore, rule 2). A
//! `Media` record tracks the upload lifecycle (`Uploading → Ready → Archived`) and lists derived
//! variants (`thumb`/`preview`).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

/// The store table media metadata records live in.
pub const MEDIA_TABLE: &str = "media";

/// The store table media chunks live in (`media_chunk:{media_id}:{n}`).
pub const CHUNK_TABLE: &str = "media_chunk";

/// The constant `kind` discriminant for `media_list`.
pub const MEDIA_KIND: &str = "media";

/// The default chunk size (1 MiB — the scope's recommendation).
pub const CHUNK_SIZE: u32 = 1024 * 1024;

/// The upload lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MediaStatus {
    #[default]
    Uploading,
    Ready,
    Archived,
}

/// A derived variant (thumbnail/preview).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaVariant {
    pub name: String,
    pub mime: String,
    pub bytes: u64,
    pub status: VariantStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VariantStatus {
    #[default]
    Pending,
    Ready,
    Failed,
}

/// A media record — metadata for a chunked-upload binary. Content bytes live as chunk records.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub id: String,
    pub mime: String,
    pub declared_bytes: u64,
    pub checksum: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    pub status: MediaStatus,
    pub chunks: u32,
    pub chunk_size: u32,
    #[serde(default)]
    pub variants: Vec<MediaVariant>,
    pub created_ts: u64,
    #[serde(default)]
    pub ready_ts: u64,
    pub kind: String,
}

impl Media {
    pub fn new(
        id: impl Into<String>,
        mime: impl Into<String>,
        declared_bytes: u64,
        checksum: impl Into<String>,
        owner: impl Into<String>,
        chunks: u32,
        chunk_size: u32,
        created_ts: u64,
    ) -> Self {
        Self {
            id: id.into(),
            mime: mime.into(),
            declared_bytes,
            checksum: checksum.into(),
            owner: owner.into(),
            origin: None,
            status: MediaStatus::Uploading,
            chunks,
            chunk_size,
            variants: Vec::new(),
            created_ts,
            ready_ts: 0,
            kind: MEDIA_KIND.to_string(),
        }
    }
}

/// How many chunks `declared_bytes` needs at `chunk_size`.
pub fn chunk_count(declared_bytes: u64, chunk_size: u32) -> u32 {
    let cs = chunk_size as u64;
    ((declared_bytes + cs - 1) / cs) as u32
}

// ── Raw store verbs ──────────────────────────────────────────────────────────────────────────

pub async fn media_write(store: &Store, ws: &str, media: &Media) -> Result<(), StoreError> {
    let value = serde_json::to_value(media).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, MEDIA_TABLE, &media.id, &value).await
}

pub async fn media_get_raw(store: &Store, ws: &str, id: &str) -> Result<Option<Media>, StoreError> {
    match read(store, ws, MEDIA_TABLE, id).await? {
        Some(v) => {
            let media: Media =
                serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(media))
        }
        None => Ok(None),
    }
}

pub async fn media_list_raw(store: &Store, ws: &str) -> Result<Vec<Media>, StoreError> {
    let rows = store_list(store, ws, MEDIA_TABLE, "kind", MEDIA_KIND).await?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| StoreError::Decode(e.to_string())))
        .collect()
}

/// Write a chunk to the store. Idempotent (re-write upserts the same row).
pub async fn chunk_write(
    store: &Store,
    ws: &str,
    media_id: &str,
    n: u32,
    bytes: &[u8],
) -> Result<(), StoreError> {
    let chunk_id = format!("{media_id}:{n}");
    let value = serde_json::json!({
        "media_id": media_id,
        "n": n,
        "bytes": base64_encode(bytes),
        "len": bytes.len(),
    });
    write(store, ws, CHUNK_TABLE, &chunk_id, &value).await
}

/// Read a chunk from the store.
pub async fn chunk_read(
    store: &Store,
    ws: &str,
    media_id: &str,
    n: u32,
) -> Result<Option<Vec<u8>>, StoreError> {
    let chunk_id = format!("{media_id}:{n}");
    match read(store, ws, CHUNK_TABLE, &chunk_id).await? {
        Some(v) => {
            let bytes_b64 = v.get("bytes").and_then(|b| b.as_str()).unwrap_or("");
            Ok(Some(base64_decode(bytes_b64)?))
        }
        None => Ok(None),
    }
}

/// Read all chunks in order and concatenate.
pub async fn read_all_bytes(store: &Store, ws: &str, media: &Media) -> Result<Vec<u8>, StoreError> {
    let mut out = Vec::with_capacity(media.declared_bytes as usize);
    for n in 0..media.chunks {
        match chunk_read(store, ws, &media.id, n).await? {
            Some(chunk) => out.extend(chunk),
            None => return Err(StoreError::Backend(format!("missing chunk {n}"))),
        }
    }
    Ok(out)
}

/// Write a variant's bytes (stored as a chunk-like record).
pub async fn variant_write(
    store: &Store,
    ws: &str,
    media_id: &str,
    variant: &str,
    bytes: &[u8],
) -> Result<(), StoreError> {
    let id = format!("{media_id}:variant:{variant}");
    let value = serde_json::json!({
        "media_id": media_id,
        "variant": variant,
        "bytes": base64_encode(bytes),
        "len": bytes.len(),
    });
    write(store, ws, CHUNK_TABLE, &id, &value).await
}

/// Read a variant's bytes.
pub async fn variant_read(
    store: &Store,
    ws: &str,
    media_id: &str,
    variant: &str,
) -> Result<Option<Vec<u8>>, StoreError> {
    let id = format!("{media_id}:variant:{variant}");
    match read(store, ws, CHUNK_TABLE, &id).await? {
        Some(v) => {
            let bytes_b64 = v.get("bytes").and_then(|b| b.as_str()).unwrap_or("");
            Ok(Some(base64_decode(bytes_b64)?))
        }
        None => Ok(None),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Decode stored chunk bytes. A corrupt row is a hard error (`StoreError::Decode`) — silently
/// serving truncated bytes with a 200 would poison caches under a stale-but-matching ETag.
fn base64_decode(s: &str) -> Result<Vec<u8>, StoreError> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(s)
        .map_err(|e| StoreError::Decode(format!("corrupt chunk (base64): {e}")))
}
