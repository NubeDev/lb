//! [`token_revoke_mark`] / [`token_revoked`] — the **live-token revoke marker** (access-console
//! scope). A per-`(workspace, subject)` tombstone RECORD the token-verify chokepoint reads on
//! every request, so a killed subject's *current* (cached) token is refused on the next verify —
//! closing the freshness-asymmetry gap the shipped [`revoke_subject`](crate::revoke_subject) leaves
//! open (that one tombstones *grants* → next re-mint; this one refuses the *live token* → next
//! request). The two COMPOSE: `revoke_tokens` calls both for a full immediate lockout.
//!
//! Shape: a single workspace-namespaced record `token_revoke:<subject-key>` whose presence (the
//! record existing at all) IS the marker — a read, O(1)-ish, keyed workspace+subject. Writing it is
//! idempotent (an upsert), so it replays cleanly under sync (§6.8): a stale synced edge re-applies
//! the same marker, never resurrects the token, and a fresh marker reaches a peer within the TTL
//! bound (worst-case window = TTL; the single-node case IS instant). We do NOT claim instant global
//! revoke — the UI states the TTL window honestly.
//!
//! Why a record and not a nonce bump or a global list: a nonce bump churns every live token in the
//! workspace on one revoke; a deny-list is an unbounded scan. A workspace+subject-keyed read is the
//! smallest blast-radius mechanism that composes with the TTL the verify path already enforces.

use lb_store::{read, write, Store, StoreError};
use serde::{Deserialize, Serialize};

use crate::subject::Subject;

/// The store table the per-subject revoke marker lives in, within a workspace namespace.
pub const TOKEN_REVOKE_TABLE: &str = "token_revoke";

/// The marker record itself — presence is the signal; the fields exist only so the row is a real
/// record (and so `write` is a stable upsert). `subject` mirrors the id for human readability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRevoke {
    pub subject: String,
}

/// Stable record id for a subject's marker — the subject's `kind:name` key (the namespace already
/// carries the workspace, so the id alone identifies the marker within a workspace).
fn marker_id(subject: &Subject) -> String {
    subject.as_key()
}

/// Write the live-token revoke marker for `subject` in workspace `ws`. Idempotent (re-marking
/// upserts the same row). After this, the verify chokepoint refuses `subject`'s current token on
/// the next request in this workspace.
pub async fn token_revoke_mark(
    store: &Store,
    ws: &str,
    subject: &Subject,
) -> Result<(), StoreError> {
    let value = serde_json::to_value(TokenRevoke {
        subject: subject.as_key(),
    })
    .map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TOKEN_REVOKE_TABLE, &marker_id(subject), &value).await
}

/// Does a live-token revoke marker exist for `subject` in workspace `ws`? The single read the
/// verify path performs per request to honor a past [`token_revoke_mark`]. Absent → the subject's
/// token is not live-revoked (its grants still apply). Never consults another workspace's marker
/// (the namespace wall, §7).
pub async fn token_revoked(store: &Store, ws: &str, subject: &Subject) -> Result<bool, StoreError> {
    Ok(read(store, ws, TOKEN_REVOKE_TABLE, &marker_id(subject))
        .await?
        .is_some())
}
