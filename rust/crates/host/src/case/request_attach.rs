//! **A file from the contractor** — the attachment write behind the public upload route
//! (case-plane scope, wave 2).
//!
//! **This is not a third capability.** The token principal's surface stays exactly two verbs
//! (`case.request.view`, `case.request.reply`); minting `mcp:case.request.attach:call` would widen
//! the smallest principal in the system for a file upload. Instead the route re-checks that the
//! token is scoped to this request, and the asset is then written under the **host's own**
//! authority — a narrowly-minted internal principal that may write exactly one asset id and nothing
//! else.
//!
//! **The asset id is one record-id segment**, minted here as a ULID and never derived from the
//! uploaded filename. A `.` in an asset id breaks `store:asset/{id}:write` — the file uploads, the
//! response looks fine, and the bytes are unreadable for ever. The original filename rides back to
//! the caller (and into the reply's event payload) as *data*, never as an identifier.
//!
//! The size ceiling is `lb_host`'s own [`MAX_ASSET_BYTES`], not a second number invented here.

use lb_auth::Principal;
use lb_store::{new_ulid, Store};
use serde::Serialize;

use super::error::CaseSvcError;
use super::request_scope::request_id_in_scope;
use crate::assets::{put_asset, MAX_ASSET_BYTES};

/// What the upload route hands back — the id the reply must reference, and what it was called.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AttachmentReceipt {
    pub id: String,
    pub name: String,
    pub size: usize,
}

/// Store `bytes` as an attachment for `request_id`, as the token principal.
///
/// `principal` is checked for scope only; the write runs under a host principal minted for this one
/// asset id. `name` and `mime` are caller-supplied data and are never used to address anything.
pub async fn case_request_attach(
    store: &Store,
    principal: &Principal,
    ws: &str,
    request_id: &str,
    name: &str,
    mime: &str,
    bytes: Vec<u8>,
    ts: u64,
) -> Result<AttachmentReceipt, CaseSvcError> {
    request_id_in_scope(principal, request_id)?;
    if bytes.is_empty() {
        return Err(CaseSvcError::BadInput("the upload was empty".into()));
    }
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(CaseSvcError::BadInput(format!(
            "the file is larger than the {MAX_ASSET_BYTES}-byte limit"
        )));
    }
    // The request must exist and be the one this token holds — checked again against the store, so
    // a scope marker alone can never mint an asset in a workspace with no such ask.
    if lb_cases::request_get(store, ws, request_id)
        .await?
        .is_none()
    {
        return Err(CaseSvcError::Denied);
    }

    let id = new_ulid();
    let writer = Principal::routed(
        // `system:` — the host wrote this, on a party's behalf. The party's own attribution lives on
        // the reply event, which is where the audit trail looks for it.
        "system:case-request",
        ws,
        vec![format!("store:asset/{id}:write")],
    );
    let size = bytes.len();
    put_asset(store, &writer, ws, &id, mime, bytes, ts)
        .await
        .map_err(|e| CaseSvcError::Store(e.to_string()))?;

    Ok(AttachmentReceipt {
        id,
        name: name.to_string(),
        size,
    })
}
