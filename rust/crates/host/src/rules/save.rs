//! `rules.save` — create/update a saved rule `rule:{ws}:{id}` (rules-engine-scope CRUD). Gated
//! `mcp:rules.save:call` (workspace-first) at the bridge; here we authorize the store-write surface
//! then upsert. Idempotent on `id`. The body is NOT executed at save (a save is not a run).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;
use serde_json::json;

use lb_rules::RuleParam;

use super::error::RulesError;
use super::record::{SavedRule, RULE_TABLE};

/// Persist a saved rule. `id` is the stable key (caller-supplied; defaults to `name`). Returns the id.
pub async fn rules_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    name: &str,
    body: &str,
    params: Vec<RuleParam>,
) -> Result<String, RulesError> {
    authorize_store_write(principal, ws)?;
    let rule = SavedRule {
        id: id.to_string(),
        name: name.to_string(),
        body: body.to_string(),
        params,
        deleted: false,
    };
    let value = serde_json::to_value(&rule).map_err(|e| RulesError::Internal(e.to_string()))?;
    lb_store::write(store, ws, RULE_TABLE, id, &value)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    let _ = json!({});
    Ok(id.to_string())
}

/// A saved rule is a store record — writing it needs the store-write surface (defense in depth below
/// the MCP gate). Workspace-first.
fn authorize_store_write(principal: &Principal, ws: &str) -> Result<(), RulesError> {
    let req = Request::new(ws, Surface::Store, "rule", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(RulesError::Denied),
    }
}
