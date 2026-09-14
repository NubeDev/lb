//! `case_members` — the drawer's citation list, paged, over the capability gate.
//!
//! **Gates on `case.get`, not a cap of its own.** The members ARE the case's detail: a reader who
//! may open the case may see which detections it covers, and a separate `mcp:case.members:call`
//! would exist in no role bundle — the shipped-but-unusable trap `tool_gate.rs` documents four
//! times. The alias `case.members → case.get` in that table is what makes the dispatcher agree.
//!
//! **This layer adds the thing `lb-cases` cannot: what each cited insight is CALLED.** The
//! membership row holds an id, so a drawer rendering the page unaided draws a column of ULIDs. The
//! title lives on the insight, in `lb-insights`, which `lb-cases` does not depend on and must not —
//! the work plane consuming the detection plane for a display string is the wrong arrow. The host
//! depends on both, so the join belongs here, beside `case_list`'s resolution of who "me" is.
//!
//! **One query, whatever the page size.** The echo is resolved by reading the case's insights
//! through `insight.case_id` — the back-reference the grouping pass writes for exactly this
//! purpose ("saves 200 extra reads", `insights::insight`) — and indexing them by id. A `get` per
//! member would be an N+1 at 200 reads a page; pushing it to the client would be the same N+1 over
//! the wire, and would demand `insight.get` caps of a reader who holds `case.get` and not that.
//!
//! **An insight that does not resolve leaves the echo absent, never blank.** A deleted insight, or
//! one whose back-reference has not been written yet, yields a row with no title — and the client
//! falls back to the id, which at least identifies the detection. Degrade, never lie.

use std::collections::HashMap;

use lb_auth::Principal;
use lb_cases::MemberPage;
use lb_insights::{Insight, INSIGHT_TABLE};
use lb_mcp::authorize_tool;
use lb_store::{list as store_list, Store};

use super::error::CaseSvcError;

/// One page of `case_id`'s members, each carrying its insight's `title` and `severity`. Gated by
/// `mcp:case.get:call`.
pub async fn case_members(
    store: &Store,
    principal: &Principal,
    ws: &str,
    case_id: &str,
    limit: usize,
    after: Option<&str>,
) -> Result<MemberPage, CaseSvcError> {
    authorize_tool(principal, ws, "case.get").map_err(|_| CaseSvcError::Denied)?;
    // Establish the case exists in THIS workspace first: without it a ws-B caller learns nothing,
    // but a caller in ws-A asking for a ws-B case id would get an empty page that reads as "a case
    // with no members" rather than "no such case".
    if lb_cases::get(store, ws, case_id).await?.is_none() {
        return Err(CaseSvcError::BadInput(format!("no such case: {case_id}")));
    }
    let mut page = lb_cases::members(store, ws, case_id, limit, after).await?;
    let titles = echo_for(store, ws, case_id).await;
    for row in &mut page.items {
        if let Some((title, severity)) = titles.get(&row.member.insight_id) {
            row.title = Some(title.clone());
            row.severity = Some(severity.clone());
        }
    }
    Ok(page)
}

/// `insight_id → (title, severity)` for every insight this case owns, in ONE store read.
///
/// Reads by the `case_id` back-reference rather than by the page's ids, because the store's
/// single-field equality read is the only batch door there is — and this is the read that
/// back-reference exists to make possible. A failure here is NOT an error: the citations are the
/// answer and the titles are a decoration on them, so a store hiccup costs the labels, not the page.
async fn echo_for(store: &Store, ws: &str, case_id: &str) -> HashMap<String, (String, String)> {
    let rows = match store_list(store, ws, INSIGHT_TABLE, "case_id", case_id).await {
        Ok(rows) => rows,
        Err(e) => {
            tracing::warn!(ws, case_id, error = %e, "case.members echo not resolved");
            return HashMap::new();
        }
    };
    rows.into_iter()
        .filter_map(|v| serde_json::from_value::<Insight>(v).ok())
        .map(|i| {
            let severity = serde_json::to_value(i.severity)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            (i.id, (i.title, severity))
        })
        .collect()
}
