//! `request_save` — persist a whole [`CaseRequest`] (cases internal).
//!
//! Every request verb reads the row, edits the struct, and writes it back; this is the one place
//! that last step lives, the same discipline [`crate::save`] holds for a case. Keeping it in one
//! file is what makes "the token hash is the record id" a fact stated once rather than a convention
//! six call sites have to remember.
//!
//! **The row id is the request id, and the `token_hash` is a field.** The invite record does it the
//! other way round (`invite:{hash}`), which is right for a record whose ONLY lookup is by token.
//! A request is looked up three ways — by id (the drawer), by case (the case's asks) and by hash
//! (the token) — so the id is the id, and the hash lookup is an equality filter on a field.

use lb_store::{write, Store};

use crate::case_request::{CaseRequest, TABLE};
use crate::error::CasesError;

/// Write `request` back at `(ws, request.id)`.
pub(crate) async fn request_save(
    store: &Store,
    ws: &str,
    request: &CaseRequest,
) -> Result<(), CasesError> {
    let value = serde_json::to_value(request).map_err(CasesError::decode)?;
    write(store, ws, TABLE, &request.id, &value).await?;
    Ok(())
}
