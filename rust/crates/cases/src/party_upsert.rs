//! `party_upsert` — write one party into the workspace roster (case-plane scope, wave 2).
//!
//! Upserts by `id`, exactly like [`crate::policy_set`]: a second write with the same id replaces the
//! row, so the settings surface edits in place and a pack re-seeds idempotently. Validation runs
//! BEFORE the write, so a rejected upsert leaves the roster untouched.
//!
//! Authorization is the host layer's job (ADMIN) — this is the raw verb.

use lb_store::{write, Store};

use crate::error::CasesError;
use crate::party::{validate_party, Party, TABLE};

/// Write `party` at `(ws, party.id)`, replacing any row with the same id.
pub async fn party_upsert(store: &Store, ws: &str, party: &Party) -> Result<(), CasesError> {
    validate_party(party)?;
    let value = serde_json::to_value(party).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &party.id, &value).await?;
    Ok(())
}
