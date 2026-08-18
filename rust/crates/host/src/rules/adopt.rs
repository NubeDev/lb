//! `rules.adopt_schedule {id}` — claim ownership of a scheduled rule's headless fire.
//!
//! **Why this verb exists.** Run-as-owner (`flows/execute_node/run_as_owner.rs`) runs a scheduled
//! rule as the subject recorded in `SavedRule::scheduled_by`, which `rules.save` captures. Rules
//! saved BEFORE that shipped carry no owner, so they keep firing on `reactor_caps()` alone — i.e.
//! they stay broken in exactly the way lb#167 describes, and no amount of waiting fixes them.
//!
//! **Why it is not a silent backfill.** The obvious migration — stamp some plausible subject onto
//! every ownerless rule — is a privilege grant performed by a migration script, which is precisely
//! the thing that must never happen quietly: it would hand a rule the authority of whoever the
//! migration happened to pick, with no human deciding that. There is also no honest source for the
//! answer; the store does not record who authored a rule before this field existed.
//!
//! So adoption is an explicit act by an authenticated caller, who becomes the owner and can only
//! ever confer their OWN authority — the same thing they would confer by re-saving the rule, which
//! is the other (equally valid) way to fix an ownerless schedule. Re-saving is the natural path when
//! you are editing anyway; this verb is for adopting a rule you do not want to modify.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;

use super::error::RulesError;
use super::record::{SavedRule, RULE_TABLE};

/// Record the caller as the owner of rule `id`'s scheduled fire. Requires rule-write (the same
/// surface `rules.save` takes — changing who a schedule runs as is a write to the rule).
///
/// Returns `(id, owner)`. Idempotent: adopting a rule you already own rewrites the same value.
/// Adopting a rule owned by someone ELSE is allowed and is the point — it is how a schedule is
/// handed over when its original owner leaves — but it is a rule-write, so it is gated and audited
/// like any other.
pub async fn rules_adopt_schedule(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(String, String), RulesError> {
    authorize_store_write(principal, ws)?;
    let val = lb_store::read(store, ws, RULE_TABLE, id)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?
        .ok_or(RulesError::NotFound)?;
    let mut rule: SavedRule =
        serde_json::from_value(val).map_err(|e| RulesError::Internal(e.to_string()))?;
    if rule.deleted {
        return Err(RulesError::NotFound);
    }
    // An unscheduled rule has no headless fire path, so an owner on it would be an identity nothing
    // ever reads. Refuse rather than store a misleading field.
    if rule.schedule.is_none() {
        return Err(RulesError::BadInput(format!(
            "rule `{id}` has no `#[schedule(...)]` directive, so it has no scheduled fire to own"
        )));
    }
    let owner = principal.sub().to_string();
    if owner.is_empty() {
        return Err(RulesError::BadInput(
            "the calling principal has no subject to record as owner".into(),
        ));
    }
    rule.scheduled_by = Some(owner.clone());
    let value = serde_json::to_value(&rule).map_err(|e| RulesError::Internal(e.to_string()))?;
    lb_store::write(store, ws, RULE_TABLE, id, &value)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    Ok((id.to_string(), owner))
}

fn authorize_store_write(principal: &Principal, ws: &str) -> Result<(), RulesError> {
    let req = Request::new(ws, Surface::Store, "rule", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(RulesError::Denied),
    }
}
