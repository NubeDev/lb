//! `chains.run {chain_id, params}` → `{run_id}` — the manual trigger. A chain is a JOB (never a
//! blocking call, §6.1 batch-as-job): we create a durable `lb-jobs` job record for the run (status +
//! resume anchor), seed the run-store, and drive the frontier. Drive is idempotent + resumable: a
//! restart re-drives from the durable per-step records (the CAS claim makes a redelivered step a
//! no-op). `chains.resume {run_id}` re-drives an interrupted run.

use std::sync::Arc;

use lb_auth::Principal;

use crate::boot::Node;
use crate::rules::RuleModel;

use super::coordinator;
use super::error::ChainsError;
use super::get::chains_get;

/// Start a manual run of chain `chain_id`. Returns the run id (the chain runs as a durable job).
#[allow(clippy::too_many_arguments)]
pub async fn chains_run(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    chain_id: &str,
    params: serde_json::Map<String, serde_json::Value>,
    model: Arc<dyn RuleModel>,
    run_id: &str,
    now: u64,
) -> Result<String, ChainsError> {
    // Read + (re)validate the chain (a stored chain was validated at save; re-check defends a hand-
    // edited record). Authorize is the chains.run cap at the bridge + chains.get's read here.
    let mut chain = chains_get(&node.store, principal, ws, chain_id).await?;
    chain.params = merge_params(chain.params, params);
    chain
        .validate(crate::rules::max_chain_steps())
        .map_err(ChainsError::from)?;

    // Create the durable job record (status anchor). Idempotent on run_id. A chain run is a job of
    // kind `chain` whose payload is the chain id (rule-chains-scope: the chain IS the batch-as-job).
    let job = lb_jobs::Job::new(run_id, "chain", chain_id, now);
    lb_jobs::create(&node.store, ws, &job)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?;

    coordinator::start(node, ws, run_id, &chain)
        .await
        .map_err(ChainsError::Internal)?;
    coordinator::drive(node, principal, ws, run_id, &chain, model, now)
        .await
        .map_err(ChainsError::Internal)?;

    lb_jobs::complete(&node.store, ws, run_id, lb_jobs::JobStatus::Done)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?;

    Ok(run_id.to_string())
}

/// Re-drive an interrupted run from its durable state (the resume path — exercised by the restart
/// test). A duplicate re-drive is a no-op (the CAS claim + finalize guard).
pub async fn chains_resume(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    chain_id: &str,
    run_id: &str,
    model: Arc<dyn RuleModel>,
    now: u64,
) -> Result<(), ChainsError> {
    let chain = chains_get(&node.store, principal, ws, chain_id).await?;
    coordinator::drive(node, principal, ws, run_id, &chain, model, now)
        .await
        .map_err(ChainsError::Internal)?;
    let _ = lb_jobs::complete(&node.store, ws, run_id, lb_jobs::JobStatus::Done).await;
    Ok(())
}

/// Overlay the run's params onto the chain's declared defaults.
fn merge_params(
    mut base: serde_json::Map<String, serde_json::Value>,
    over: serde_json::Map<String, serde_json::Value>,
) -> serde_json::Map<String, serde_json::Value> {
    for (k, v) in over {
        base.insert(k, v);
    }
    base
}

/// Coerce a JSON object of params into a serde map (helper for the bridge).
pub fn params_map(v: &serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    v.as_object().cloned().unwrap_or_default()
}
