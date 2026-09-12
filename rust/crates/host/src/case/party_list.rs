//! `party.list` — read the workspace's party roster (case-plane scope, wave 2).
//!
//! **Admin**, the read half of `party.upsert`. Deliberately NOT a viewer read: the roster is a list
//! of external email addresses and phone numbers belonging to other companies, and "anyone who can
//! read the queue can read every contractor's contact details" is not a trade this plane makes. A
//! member does not need it to work — the send verb resolves the party by id.
//!
//! Optional `kind` / `site` narrowing is passed straight through; both values are opaque workspace
//! strings (rule 10).

use lb_auth::Principal;
use lb_cases::{Party, PartyKind};
use lb_mcp::authorize_tool;
use lb_store::Store;

use super::error::CaseSvcError;

/// Read `ws`'s roster as `principal`, optionally narrowed.
pub async fn case_party_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
    kind: Option<PartyKind>,
    site: Option<&str>,
    include_disabled: bool,
) -> Result<Vec<Party>, CaseSvcError> {
    authorize_tool(principal, ws, "party.list").map_err(|_| CaseSvcError::Denied)?;
    Ok(lb_cases::party_list(store, ws, kind, site, include_disabled).await?)
}
