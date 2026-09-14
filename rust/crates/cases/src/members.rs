//! `members` — read a case's citation list, PAGED (case-plane scope).
//!
//! Paged because a storm case is hundreds of rows — the reason membership is its own table rather
//! than an array on the case. Ordered by `insight_id` so the cursor is the last id of the page: a
//! stable total order that does not shift when a member is added mid-walk.
//!
//! Rides `case.get`'s capability — no new read cap.
//!
//! **The page carries a read-time ECHO of each cited insight's `title` and `severity`**
//! ([`MemberRow`]), and this crate leaves both `None`. It cannot fill them: the insight plane is
//! `lb-insights`, which this crate does not depend on and must not — the arrow would make the work
//! plane a consumer of the detection plane for a display string. The HOST fills them, in one query,
//! which is the same division `case_list` already draws for the `Mine` lane ("the one thing that
//! layer adds is who *me* is"). See `host/src/case/members.rs`.

use lb_store::{list as store_list, Store};
use serde::{Deserialize, Serialize};

use crate::case_member::{CaseMember, TABLE};
use crate::error::CasesError;

/// One citation as the drawer reads it: the stored membership plus the host's read-time echo of the
/// insight it cites.
///
/// **The echo is not stored.** `title` and `severity` belong to the insight; a copy on the
/// membership row would go stale the moment the insight was retitled, and nothing would ever fix
/// it. Resolved on read instead — exactly as [`crate::CaseRow`] carries a `member_count` the case
/// record does not hold.
///
/// Both fields are `Option` and both are skipped when absent, so a client that predates the echo
/// sees the byte-for-byte page it saw before and a client that expects it falls back to the id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberRow {
    #[serde(flatten)]
    pub member: CaseMember,
    /// The cited insight's headline. `None` when the insight is gone, or when the reader is this
    /// crate (which cannot resolve it — see the module doc).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The cited insight's severity, as its wire string. `None` on the same terms as `title`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub severity: Option<String>,
}

impl MemberRow {
    /// A row with no echo — what this crate returns and what the host then fills.
    pub fn bare(member: CaseMember) -> Self {
        Self {
            member,
            title: None,
            severity: None,
        }
    }
}

/// One page of a case's members.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemberPage {
    /// The members, ordered by `insight_id`.
    pub items: Vec<MemberRow>,
    /// The `insight_id` to pass as `after` for the next page. `None` at the end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<String>,
    /// How many members the case has in TOTAL — the number the roster renders. Carried on the page
    /// so a drawer never has to walk every page to answer "how many detections is this".
    pub total: usize,
}

/// The most members one page may carry. A ceiling; a larger `limit` is clamped, not refused.
pub const MAX_MEMBER_PAGE: usize = 200;

/// Every member of `case_id`, ordered by `insight_id`, with NO echo and no paging.
///
/// The read for a caller that wants the MEMBERSHIP rather than the drawer's page: merge and split
/// (which move the whole set), and the host's echo walks. Distinguishing the two is not tidiness —
/// [`members`] costs a second query in the host to resolve titles nobody on those paths reads, and a
/// caller asking for `MAX_MEMBER_PAGE` with no cursor was only ever spelling "all of them".
pub async fn members_all(
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
    let mut items: Vec<MemberRow> = all
        .into_iter()
        .filter(|m| after.is_none_or(|a| m.insight_id.as_str() > a))
        .map(MemberRow::bare)
        .collect();
    let next = if items.len() > limit {
        items.truncate(limit);
        items.last().map(|m| m.member.insight_id.clone())
    } else {
        None
    };
    Ok(MemberPage { items, next, total })
}
