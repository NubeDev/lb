//! `party.upsert` — write one party into the workspace roster (case-plane scope, wave 2).
//!
//! **Admin**, beside `policy.sla.set` and for the same reason: the roster decides who the platform
//! will email on the workspace's behalf. Adding a row here is adding an address that will receive
//! mail signed with the customer's name and a link that grants access to a case's evidence — that
//! is administration, not authoring. A member raises the ask; an admin decides who may be asked.
//!
//! Upserts by `id`: a second write with the same id replaces the row, so the settings surface edits
//! in place and a pack re-seeds idempotently. The gate runs BEFORE validation, so a caller without
//! the cap learns nothing about whether their payload would have been accepted.
//!
//! ## Why the wire argument is its own type
//!
//! [`PartyInput`] is `deny_unknown_fields`; the stored [`Party`] record is not. That split is
//! deliberate, and it is the whole reason this type exists rather than decoding straight into the
//! record:
//!
//! - **At the door, an unknown key is a mistake.** `{"email": "…"}` at the top level instead of
//!   under `contact` used to be accepted in silence: the write returned `{ "id": … }`, the address
//!   went nowhere, and the roster held a contractor nobody could reach — the failure surfaced hours
//!   later at `case.request.send`, to a different person, with no clue what had happened. A 200
//!   that quietly drops what you sent is the worst answer available.
//! - **In the store, an unknown key is the future.** `Party` IS the persisted row. Putting the deny
//!   on it would mean a node that predates a field added later cannot decode a row a newer node
//!   wrote — the additive `#[serde(default)]` discipline every other record here follows exists
//!   precisely so that never happens.
//!
//! `ts` is accepted and ignored: the MCP bridge passes the caller's whole argument object, every
//! other case verb takes a logical clock in it, and refusing a party upsert because it carried a
//! harmless `ts` would be pedantry rather than protection.

use lb_auth::Principal;
use lb_cases::{Contact, Party, PartyKind};
use lb_mcp::authorize_tool;
use lb_store::Store;
use serde::Deserialize;

use super::error::CaseSvcError;

/// The wire argument for `party.upsert` — the same fields as [`Party`], and NOTHING else.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartyInput {
    pub id: String,
    pub kind: PartyKind,
    pub name: String,
    #[serde(default)]
    pub contact: Contact,
    #[serde(default)]
    pub sites: Vec<String>,
    #[serde(default)]
    pub trades: Vec<String>,
    #[serde(default)]
    pub default_ask_window_h: u32,
    /// Accepted and ignored — see the module note.
    #[serde(default)]
    pub ts: Option<u64>,
}

impl From<PartyInput> for Party {
    fn from(input: PartyInput) -> Self {
        Party {
            id: input.id,
            kind: input.kind,
            name: input.name,
            contact: input.contact,
            sites: input.sites,
            trades: input.trades,
            default_ask_window_h: input.default_ask_window_h,
        }
    }
}

/// Write `party` into `ws`'s roster, replacing any row with the same id.
pub async fn case_party_upsert(
    store: &Store,
    principal: &Principal,
    ws: &str,
    party: &Party,
) -> Result<(), CaseSvcError> {
    authorize_tool(principal, ws, "party.upsert").map_err(|_| CaseSvcError::Denied)?;
    lb_cases::party_upsert(store, ws, party).await?;
    Ok(())
}
