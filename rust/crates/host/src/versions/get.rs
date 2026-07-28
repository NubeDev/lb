//! `versions.get { kind, id, version_id }` — ONE snapshot, fetched lazily by a caller that has
//! already seen the metadata (`versions.list`) and picked a version to inspect or restore.
//!
//! Its own grant (`mcp:versions.get:call`), not the list's: a snapshot is the full record content,
//! while the list is provenance. They are different reads even though both are reads.

use lb_auth::Principal;
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::VersionsError;
use super::list::unknown_kind;
use super::plan::plan_for_kind;
use super::record::EntityVersion;
use super::store::read_version;

pub async fn versions_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    kind: &str,
    id: &str,
    version_id: &str,
) -> Result<EntityVersion, VersionsError> {
    authorize_tool(principal, ws, "versions.get").map_err(|_| VersionsError::Denied)?;
    plan_for_kind(kind).ok_or_else(|| unknown_kind(kind))?;
    read_version(store, ws, kind, id, version_id)
        .await?
        .ok_or(VersionsError::NotFound)
}
