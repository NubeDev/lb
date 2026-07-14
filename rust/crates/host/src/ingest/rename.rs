//! `series.rename` — the host's capability chokepoint for renaming a series (data-console scope:
//! series lifecycle). Gates first (`mcp:series.rename:call`, workspace-first §3.6 then capability
//! §3.5), then calls the raw `lb_ingest::rename_series`, which carries the series' samples, rollups,
//! staged rows, registry row, and tag edges from the old name to the new one in `ws`.
//!
//! A rename into an already-used name is refused (no silent merge) — surfaced as `BadInput`, not a
//! denial (it is a client error about the target, not an authorization failure). A denial stays
//! opaque. Destructive footprint, so it carries its own cap distinct from `ingest.write`.

use lb_auth::Principal;
use lb_ingest::{rename_series, RenameError};
use lb_store::Store;

use super::authorize::authorize_ingest;
use super::error::IngestError;

/// Rename `from` → `to` in `ws`, as `principal`. Authorizes `mcp:series.rename:call` first. Refuses
/// (as `BadInput`) if `to` already exists or equals `from`; never merges two series.
pub async fn series_rename(
    store: &Store,
    principal: &Principal,
    ws: &str,
    from: &str,
    to: &str,
) -> Result<(), IngestError> {
    authorize_ingest(principal, ws, "series.rename")?;
    rename_series(store, ws, from, to)
        .await
        .map_err(|e| match e {
            RenameError::TargetExists(_) | RenameError::Unchanged => {
                IngestError::BadInput(e.to_string())
            }
            RenameError::Store(s) => IngestError::Store(s),
        })
}
