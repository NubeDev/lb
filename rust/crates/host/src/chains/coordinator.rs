//! The coordinator — `start` + `drive` (rule-chains-scope, ported from rubix-cube's
//! `WorkflowCoordinator`). `start` seeds the run + enqueues the in-degree-0 frontier; `drive` runs the
//! ready frontier (each step: CAS-claim → resolve bindings → run the saved rule via `lb-rules` →
//! record outcome → release dependents / fan-in / apply failure policy), looping until the frontier
//! exhausts, then finalizes. The durable per-step records + the CAS claim make a restart resume the
//! un-run steps exactly once (a duplicate redelivery no-ops) — the headline offline/sync property.

use std::sync::Arc;

use lb_auth::Principal;
use lb_rules::workflow::{Chain, FailurePolicy, Outcome, StepRecord};

use crate::boot::Node;
use crate::rules::{rules_run, RuleModel};

use super::record::ClaimState;
use super::run_store;

/// Seed the run (Pending + per-step state) and return the in-degree-0 frontier to drive.
pub async fn start(node: &Arc<Node>, ws: &str, run_id: &str, chain: &Chain) -> Result<(), String> {
    run_store::create_run(&node.store, ws, run_id, chain).await
}

/// Drive the run to completion. Idempotent + resumable: re-driving reads the durable per-step state,
/// claims only un-run ready steps (CAS), and finalizes when every step is terminal. Returns when the
/// frontier exhausts (the run reached a terminal status).
pub async fn drive(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    chain: &Chain,
    model: Arc<dyn RuleModel>,
    now: u64,
) -> Result<(), String> {
    loop {
        // Gather the currently-ready frontier from the durable state.
        let ready = ready_frontier(node, ws, run_id, chain).await?;
        if ready.is_empty() {
            break;
        }
        for step_id in ready {
            run_one_step(
                node,
                principal,
                ws,
                run_id,
                chain,
                &step_id,
                model.clone(),
                now,
            )
            .await?;
        }
        // After releasing dependents the loop re-reads the frontier; finalize when nothing remains.
        if run_store::finalize_if_complete(&node.store, ws, chain, run_id)
            .await?
            .is_some()
        {
            break;
        }
    }
    Ok(())
}

/// The set of `Enqueued` (ready) step ids from the durable state.
async fn ready_frontier(
    node: &Arc<Node>,
    ws: &str,
    run_id: &str,
    chain: &Chain,
) -> Result<Vec<String>, String> {
    let mut ready = Vec::new();
    for step in &chain.steps {
        if let Some(rec) = run_store::read_step(&node.store, ws, run_id, &step.id).await? {
            if rec.claim == ClaimState::Enqueued {
                ready.push(step.id.clone());
            }
        }
    }
    Ok(ready)
}

/// Claim + run one step, then release its dependents / prune on failure.
#[allow(clippy::too_many_arguments)]
async fn run_one_step(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    chain: &Chain,
    step_id: &str,
    model: Arc<dyn RuleModel>,
    now: u64,
) -> Result<(), String> {
    // CAS: only the winner runs the rule (redelivery no-op).
    if !run_store::claim_step(&node.store, ws, run_id, step_id).await? {
        return Ok(());
    }
    let step = chain
        .steps
        .iter()
        .find(|s| s.id == step_id)
        .ok_or_else(|| format!("step {step_id} not in chain"))?;

    // Resolve the step's bindings against recorded upstream outputs + params.
    let inputs = run_store::resolve_bindings(&node.store, ws, chain, run_id, &step.with).await?;

    // Run the saved rule the step names, via the single-rule engine — under the caller's authority
    // (caller ∩ grant inside every collect; the chain cannot widen beyond its principal).
    let attempts = step.retry.map(|r| r.max + 1).unwrap_or(1);
    let mut last_err = None;
    let mut outcome = None;
    for _ in 0..attempts {
        match rules_run(
            node,
            principal,
            ws,
            None,
            Some(step.rule.clone()),
            inputs.clone(),
            model.clone(),
            now,
        )
        .await
        {
            Ok(result) => {
                outcome = Some(Outcome::Ok(result.output, result.findings));
                break;
            }
            Err(e) => last_err = Some(e),
        }
    }
    let outcome = outcome.unwrap_or_else(|| {
        Outcome::Err(
            last_err
                .map(|e| e.to_string())
                .unwrap_or_else(|| "step failed".into()),
        )
    });
    let failed = matches!(outcome, Outcome::Err(_));

    run_store::record_outcome(
        &node.store,
        ws,
        run_id,
        step_id,
        &StepRecord {
            outcome,
            attempts,
            ms: 0,
        },
    )
    .await?;

    // Failure policy: Halt prunes the failed step's subtree; Continue (and any success) releases
    // dependents (a failed-under-Continue upstream resolves to null downstream — the binding rule).
    if failed && chain.failure_policy == FailurePolicy::Halt {
        run_store::skip_subtree(&node.store, ws, chain, run_id, step_id).await?;
    } else {
        let _ready = run_store::ready_dependents(&node.store, ws, chain, run_id, step_id).await?;
    }
    Ok(())
}
