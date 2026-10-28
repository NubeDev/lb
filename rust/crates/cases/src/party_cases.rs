//! `party_cases` — the cases a party still has an open request on: what the `party` filter axis
//! narrows to (Insights review 2026-10, "Waiting On: Spark Electrical"). The request plane records
//! which party was asked about which case; a request still `sent` or `opened` is one the party has
//! not answered, so the case is waiting on them.
//!
//! One responsibility: a party to the case ids it holds open requests on.

use std::collections::BTreeSet;

use lb_store::{list as store_list, Store};

use crate::case_request::{CaseRequest, TABLE};
use crate::error::CasesError;

/// The ids of the cases `party_id` has a live (unanswered, unwithdrawn) request on.
pub async fn party_cases(
    store: &Store,
    ws: &str,
    party_id: &str,
) -> Result<BTreeSet<String>, CasesError> {
    let mut out = BTreeSet::new();
    for value in store_list(store, ws, TABLE, "party_id", party_id).await? {
        let req: CaseRequest = serde_json::from_value(value).map_err(CasesError::decode)?;
        if req.status.is_live() {
            out.insert(req.case_id);
        }
    }
    Ok(out)
}
