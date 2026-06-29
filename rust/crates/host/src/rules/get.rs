//! `rules.get {id}` / `rules.list {filter?}` — workspace-scoped reads of saved rules. Gated at the
//! bridge (`mcp:rules.get`/`mcp:rules.list`); here we authorize the store-read surface then read. A
//! ws-B caller reads only ws-B rules (namespace wall) — the mandatory isolation property.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;

use super::error::RulesError;
use super::record::{SavedRule, RULE_TABLE};

/// Read one saved rule by id. `None`-equivalent is `NotFound`.
pub async fn rules_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<SavedRule, RulesError> {
    authorize_store_read(principal, ws)?;
    let val = lb_store::read(store, ws, RULE_TABLE, id)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?
        .ok_or(RulesError::NotFound)?;
    let rule: SavedRule =
        serde_json::from_value(val).map_err(|e| RulesError::Internal(e.to_string()))?;
    if rule.deleted {
        return Err(RulesError::NotFound);
    }
    Ok(rule)
}

/// List saved rules in the workspace (all; an optional name filter is applied by the caller). The
/// scan stays namespace-walled.
pub async fn rules_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<SavedRule>, RulesError> {
    authorize_store_read(principal, ws)?;
    let page = lb_store::scan(store, ws, RULE_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    let mut out = Vec::new();
    for row in page.rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope (the same one `read`
        // unwraps for `rules_get`); `scan` returns the whole record, so unwrap it before decoding —
        // otherwise every row silently fails deser and the roster is always empty (mirrors the
        // `scan_dashboards` envelope unwrap in `dashboard/store.rs`).
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        if let Ok(rule) = serde_json::from_value::<SavedRule>(inner) {
            if !rule.deleted {
                out.push(rule);
            }
        }
    }
    Ok(out)
}

fn authorize_store_read(principal: &Principal, ws: &str) -> Result<(), RulesError> {
    let req = Request::new(ws, Surface::Store, "rule", Action::Read);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(RulesError::Denied),
    }
}
