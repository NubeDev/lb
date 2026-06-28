//! `rules.delete {id}` — tombstone-upsert a saved rule (idempotent, §6.8 sync-safe). A delete of an
//! absent/already-deleted rule is a no-op, not an error. Gated `mcp:rules.delete:call` at the bridge.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;

use super::error::RulesError;
use super::record::{SavedRule, RULE_TABLE};

/// Soft-delete rule `id` in `ws`. Idempotent.
pub async fn rules_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), RulesError> {
    authorize_store_write(principal, ws)?;
    let existing = lb_store::read(store, ws, RULE_TABLE, id)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    let Some(val) = existing else {
        return Ok(()); // absent → no-op
    };
    let mut rule: SavedRule =
        serde_json::from_value(val).map_err(|e| RulesError::Internal(e.to_string()))?;
    if rule.deleted {
        return Ok(());
    }
    rule.deleted = true;
    let value = serde_json::to_value(&rule).map_err(|e| RulesError::Internal(e.to_string()))?;
    lb_store::write(store, ws, RULE_TABLE, id, &value)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    Ok(())
}

fn authorize_store_write(principal: &Principal, ws: &str) -> Result<(), RulesError> {
    let req = Request::new(ws, Surface::Store, "rule", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(RulesError::Denied),
    }
}
