//! [`revoke_subject`] — the **revoke seam** the `admin-crud` destructive verbs call when deleting a
//! user or team, so revocation-on-delete is defined in *one* place rather than reimplemented per
//! caller (admin-crud scope: "deletes that strip access call the existing authz revoke").
//!
//! It tombstones every live grant the subject holds — direct caps and role assignments alike — in
//! workspace `ws`. Idempotent: a subject with no grants revokes to a no-op; re-running double-
//! revokes harmlessly (each grant is already a tombstone). Like every revoke here it leaves
//! tombstones (not deletes) so the change replays idempotently under sync (§6.8) and a stale synced
//! grant can't resurrect access.
//!
//! Per the freshness asymmetry ([`resolve_caps`](crate::resolve_caps)): this drops the subject's
//! Gate-2 caps on the *next re-mint*. A true immediate lockout also needs `user.disable` (kills
//! minting) — the admin-crud user verbs pair the two.

use lb_store::{Store, StoreError};

use crate::grant::{grant_list, grant_revoke};
use crate::subject::Subject;

/// Revoke every grant `subject` holds in workspace `ws`. Returns the number of caps revoked (for
/// the caller's consequence/audit note). Idempotent.
pub async fn revoke_subject(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<usize, StoreError> {
    let caps = grant_list(store, ws, subject).await?;
    for cap in &caps {
        grant_revoke(store, ws, subject, cap).await?;
    }
    Ok(caps.len())
}
