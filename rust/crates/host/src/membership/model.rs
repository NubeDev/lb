//! The membership view type — the roster row the gateway / People tab reads: `sub` + `joined_ts` +
//! an optional display name (resolved from the identity directory).

use serde::{Deserialize, Serialize};

/// One effective member of a workspace — the People-tab roster row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MembershipView {
    pub sub: String,
    pub joined_ts: u64,
    /// The identity's display name, if one is resolved (else absent). Non-unique.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}
