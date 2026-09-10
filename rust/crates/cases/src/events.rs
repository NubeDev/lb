//! `events` — read a case's history, newest-first and PAGED (case-plane scope).
//!
//! Paged where the insight comment thread is not, and for the opposite reason: comments are capped
//! at 200 and never evict, while a case's history takes a row for every nudge, every reply and
//! every transition over a job that can run for months. The drawer wants the last screenful.
//!
//! Rides `case.get`'s capability — no new read cap (the history is part of the case's detail).

use lb_store::{list as store_list, Store};
use serde::{Deserialize, Serialize};

use crate::case_event::{CaseEvent, TABLE};
use crate::error::CasesError;

/// One page of a case's history, newest-first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventPage {
    /// The events, newest-first by `seq`.
    pub items: Vec<CaseEvent>,
    /// The `seq` to pass as `after` for the next (older) page. `None` at the end of the history.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<u64>,
}

/// The most events one page may carry. A hard ceiling, not a default — a caller asking for more is
/// clamped rather than refused (the page is a view, not a contract about the whole history).
pub const MAX_EVENT_PAGE: usize = 200;

/// Every event on `case_id`, newest-first. Internal: [`crate::event_append`] needs the whole set to
/// assign the next `seq`, and the paged read below is built on it.
pub(crate) async fn events_all(
    store: &Store,
    ws: &str,
    case_id: &str,
) -> Result<Vec<CaseEvent>, CasesError> {
    let rows = store_list(store, ws, TABLE, "case_id", case_id).await?;
    let mut items: Vec<CaseEvent> = rows
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();
    items.sort_by_key(|e| std::cmp::Reverse(e.seq));
    Ok(items)
}

/// One page of `case_id`'s history, newest-first, starting strictly BELOW `after` (a `seq` from the
/// previous page's `next`). `limit` is clamped to [`MAX_EVENT_PAGE`].
pub async fn events(
    store: &Store,
    ws: &str,
    case_id: &str,
    limit: usize,
    after: Option<u64>,
) -> Result<EventPage, CasesError> {
    let limit = limit.clamp(1, MAX_EVENT_PAGE);
    let all = events_all(store, ws, case_id).await?;
    let mut items: Vec<CaseEvent> = all
        .into_iter()
        .filter(|e| after.is_none_or(|a| e.seq < a))
        .collect();
    // One row past the page tells us whether there IS a next page without a second read.
    let next = if items.len() > limit {
        items.truncate(limit);
        items.last().map(|e| e.seq)
    } else {
        None
    };
    Ok(EventPage { items, next })
}
