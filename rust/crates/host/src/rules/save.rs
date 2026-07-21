//! `rules.save` — create/update a saved rule `rule:{ws}:{id}` (rules-engine-scope CRUD). Gated
//! `mcp:rules.save:call` (workspace-first) at the bridge; here we authorize the store-write surface
//! then upsert. Idempotent on `id`. The body is NOT executed at save (a save is not a run).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_reminders::is_valid;
use lb_rules::{extract_schedule, RuleParam, RuleSchedule};
use lb_store::Store;

use super::error::RulesError;
use super::record::{SavedRule, RULE_TABLE};

/// Persist a saved rule. `id` is the stable key (caller-supplied; defaults to `name`). The body is
/// **not executed** (a save is not a run); the only thing read from it is the top-of-body
/// `#[schedule(...)]` directive (scheduled-rules-scope), compiled to a [`RuleSchedule`] and stored on
/// the record. Returns the id + the compiled schedule so the caller can run the managed-flow syncer.
///
/// The directive compile is a pure `phrase → cron string` step ([`extract_schedule`]); the emitted
/// cron is validated here with `croner` (the ONE engine, via `lb-reminders::is_valid`) so a phrase that
/// compiles to an invalid spec is a **save error**, not a silently-dead schedule. Parsing happens
/// before the write and never runs the cage.
pub async fn rules_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
    name: &str,
    body: &str,
    params: Vec<RuleParam>,
) -> Result<(String, Option<RuleSchedule>), RulesError> {
    authorize_store_write(principal, ws)?;
    let schedule = compile_body_schedule(body)?;
    let rule = SavedRule {
        id: id.to_string(),
        name: name.to_string(),
        body: body.to_string(),
        params,
        schedule: schedule.clone(),
        deleted: false,
    };
    let value = serde_json::to_value(&rule).map_err(|e| RulesError::Internal(e.to_string()))?;
    lb_store::write(store, ws, RULE_TABLE, id, &value)
        .await
        .map_err(|e| RulesError::Internal(e.to_string()))?;
    Ok((id.to_string(), schedule))
}

/// Extract + compile the body's `#[schedule(...)]` directive, then croner-validate the emitted cron.
/// `Ok(None)` = no directive (run-on-demand). A malformed directive / unparseable phrase / a compiled
/// cron `croner` rejects all map to [`RulesError::BadInput`] — a clear save error naming the problem.
fn compile_body_schedule(body: &str) -> Result<Option<RuleSchedule>, RulesError> {
    let sched = extract_schedule(body).map_err(|e| RulesError::BadInput(e.to_string()))?;
    if let Some(s) = &sched {
        if !is_valid(&s.cron) {
            return Err(RulesError::BadInput(format!(
                "schedule compiled to an invalid cron expression `{}` (from {:?})",
                s.cron, s.raw
            )));
        }
    }
    Ok(sched)
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
