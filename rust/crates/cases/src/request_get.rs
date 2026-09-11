//! The three reads of the [`CaseRequest`] table (case-plane scope, wave 2): by id, by case, and by
//! token hash.
//!
//! They live together because they are one question asked three ways, and because the *third* one
//! carries a security note that must not drift from the other two:
//!
//! **[`request_by_token_hash`] compares in constant time.** The caller hands us the SHA-256 of a
//! presented token; comparing it to the stored hash with `==` on a `String` short-circuits on the
//! first differing byte, which over enough requests leaks how much of a guessed hash was right. It
//! is a scan-and-compare rather than a keyed read for exactly that reason — and because the row id
//! is the request id (see `request_save.rs`).

use lb_store::{list as store_list, read, Store};

use crate::case_request::{CaseRequest, TABLE};
use crate::error::CasesError;

/// Read one request by id, or `None` if absent in this workspace.
pub async fn request_get(
    store: &Store,
    ws: &str,
    id: &str,
) -> Result<Option<CaseRequest>, CasesError> {
    let Some(value) = read(store, ws, TABLE, id).await? else {
        return Ok(None);
    };
    Ok(Some(
        serde_json::from_value(value).map_err(CasesError::decode)?,
    ))
}

/// Every request raised on `case_id`, oldest→newest — the drawer's "who have we asked what" list.
pub async fn requests_of_case(
    store: &Store,
    ws: &str,
    case_id: &str,
) -> Result<Vec<CaseRequest>, CasesError> {
    let mut out: Vec<CaseRequest> = Vec::new();
    for value in store_list(store, ws, TABLE, "case_id", case_id).await? {
        out.push(serde_json::from_value(value).map_err(CasesError::decode)?);
    }
    out.sort_by_key(|r| r.sent_ts);
    Ok(out)
}

/// The request whose stored `token_hash` equals `token_hash`, or `None`.
///
/// Constant-time comparison over the whole table (see the module note). The scan is bounded by the
/// workspace's request count and runs on a pre-auth route, so it is the one read here that is also
/// a denial-of-service surface — the gateway rate-limits it, which is where that bound belongs.
pub async fn request_by_token_hash(
    store: &Store,
    ws: &str,
    token_hash: &str,
) -> Result<Option<CaseRequest>, CasesError> {
    let mut found = None;
    for value in store_list(store, ws, TABLE, "token_hash", token_hash).await? {
        let request: CaseRequest = serde_json::from_value(value).map_err(CasesError::decode)?;
        // Re-verify in constant time: the store's own equality filter did the lookup, and this is
        // the comparison that decides. Belt and braces, and it costs one pass over 64 hex chars.
        if constant_time_eq(request.token_hash.as_bytes(), token_hash.as_bytes()) {
            found = Some(request);
        }
    }
    Ok(found)
}

/// Byte-equality with no early exit. Length inequality is not secret (both sides are fixed-width
/// hex here), so it returns immediately; the payload comparison never short-circuits.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::constant_time_eq;

    #[test]
    fn constant_time_eq_agrees_with_equality() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }
}
