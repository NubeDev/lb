//! `members` — read a case's citation list, PAGED (case-plane scope).
//!
//! Paged because a storm case is hundreds of rows — the reason membership is its own table rather
//! than an array on the case. Ordered by `insight_id` so the cursor is the last id of the page: a
//! stable total order that does not shift when a member is added mid-walk.
//!
//! Rides `case.get`'s capability — no new read cap.

use lb_store::{list as store_list, Store};
use serde::{Deserialize, Serialize};

use crate::case_member::{CaseMember, TABLE};
use crate::error::CasesError;

/// One page of a case's members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberPage {
    /// The members, ordered by `insight_id`.
    pub items: Vec<CaseMember>,
    /// The `insight_id` to pass as `after` for the next page. `None` at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// How many members the case has in TOTAL — the number the roster renders. Carried on the page
    /// so a drawer never has to walk every page to answer "how many detections is this".
    pub total: usize,
}

/// The most members one page may carry. A ceiling; a larger `limit` is clamped, not refused.
pub const MAX_MEMBER_PAGE: usize = 200;

/// Every member of `case_id`, ordered by `insight_id`. Internal — the paged read and the
/// merge/split verbs (which move the whole set) are built on it.
pub(crate) async fn members_all(
    store: &Store,
    ws: &str,
    case_id: &str,
) -> Result<Vec<CaseMember>, CasesError> {
    let rows = store_list(store, ws, TABLE, "case_id", case_id).await?;
    let mut items: Vec<CaseMember> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    items.sort_by(|a, b| a.insight_id.cmp(&b.insight_id));
    Ok(items)
}

/// How many insights `case_id` cites. The roster's member **count** — the case-plane scope is
/// explicit that `case.list` returns this and never the members themselves.
pub async fn member_count(store: &Store, ws: &str, case_id: &str) -> Result<usize, CasesError> {
    Ok(members_all(store, ws, case_id).await?.len())
}

/// One page of `case_id`'s members, ordered by `insight_id`, starting strictly AFTER `after`.
pub async fn members(
    store: &Store,
    ws: &str,
    case_id: &str,
    limit: usize,
    after: Option<&str>,
) -> Result<MemberPage, CasesError> {
    let limit = limit.clamp(1, MAX_MEMBER_PAGE);
    let all = members_all(store, ws, case_id).await?;
    let total = all.len();
    let mut items: Vec<CaseMember> = all
        .into_iter()
        .filter(|m| after.is_none_or(|a| m.insight_id.as_str() > a))
        .collect();
    let next = if items.len() > limit {
        items.truncate(limit);
        items.last().map(|m| m.insight_id.clone())
    } else {
        None
    };
    Ok(MemberPage { items, next, total })
}
