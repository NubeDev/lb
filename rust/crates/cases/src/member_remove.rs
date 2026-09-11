//! `member_remove` — drop one citation (case-plane scope).
//!
//! Idempotent: removing a membership that is not there is a `false` success, not an error — a
//! retried merge must not fail halfway through moving a storm's worth of members.
//!
//! **This verb does NOT protect `human_placed`.** It cannot: it is the primitive [`crate::merge`]
//! and [`crate::split`] use to MOVE a member, and a person moving their own placement is exactly
//! what those verbs are for. The stop sign lives one level up, in the reactors
//! (`host/src/case/group.rs`), which check `human_placed` before they ever call this.

use lb_store::{delete, Store};

use crate::case_member::{member_id, TABLE};
use crate::error::CasesError;

/// Remove `insight_id` from `case_id` in workspace `ws`. Returns whether a row was actually there.
pub async fn member_remove(
    store: &Store,
    ws: &str,
    case_id: &str,
    insight_id: &str,
) -> Result<bool, CasesError> {
    let existed = crate::find_by_insight::memberships_of_insight(store, ws, insight_id)
        .await?
        .iter()
        .any(|m| m.case_id == case_id);
    delete(store, ws, TABLE, &member_id(case_id, insight_id)).await?;
    Ok(existed)
}
