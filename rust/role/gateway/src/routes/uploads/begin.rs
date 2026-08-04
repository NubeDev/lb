//! `POST /uploads/{sink}` — open (or **re-open**) an upload. `201 {id, offset}`.
//!
//! **Resume identity is the digest.** A `begin` carrying a `digest_hex` the sink already holds a
//! partial for returns the EXISTING handle and its offset, not a fresh id — so a client that lost
//! its id (browser refresh, new session) resumes instead of double-filling the backend's disk with a
//! second partial of the same artifact. That decision belongs to the sink; lb just carries the
//! digest and reports whatever handle comes back.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use lb_host::{UploadError, UploadMeta};

use super::gate;
use crate::state::Gateway;

/// `POST /uploads/{sink}` with `{size, digest_hex?, meta{…}}`.
pub async fn begin_upload(
    State(gw): State<Gateway>,
    headers: HeaderMap,
    Path(sink): Path<String>,
    Json(meta): Json<UploadMeta>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, String)> {
    let (s, _p) = gate::resolve(&gw, &headers, &sink).await?;
    // Bounded by config, never unlimited: the DECLARED size is checked against the sink's ceiling
    // before a single byte is accepted, so an oversized artifact is refused at the cheapest possible
    // moment instead of after gigabytes have landed on the backend's disk.
    let limit = s.max_upload_bytes();
    if meta.size > limit {
        return Err(gate::status(&UploadError::TooLarge { limit }));
    }
    let handle = s.begin(&meta).await.map_err(|e| gate::status(&e))?;
    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": handle.id, "offset": handle.offset })),
    ))
}
