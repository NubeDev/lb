//! The rule-run worker — drive one job-backed rule evaluation to a settle
//! (long-running-rules-scope). Loads the job, folds the persisted checkpoints, evaluates the body
//! on a blocking thread under the JOB governor profile with a shared [`RunControl`], then writes
//! the honest terminal/parked status: `Done` (+ result + alert fan-out), `Suspended` (pause),
//! `Cancelled`, or `Failed` (+ error). The registry entry is live exactly while the eval runs.

use std::sync::Arc;

use lb_auth::Principal;
use lb_jobs::JobStatus;
use lb_rules::{JobBinding, Rule, RuleEngine, RuleError, RuleRun, RunOptions};
use serde_json::json;

use crate::boot::Node;

use super::super::config::{job_ai_limits, job_max_writes, job_rule_limits};
use super::super::run::{build_seams, route_alerts, RunResult};
use super::super::seam::HostAiSeam;
use super::payload::{fold_checkpoints, RuleJobPayload, ERROR_KEY, RESULT_KEY};
use super::seam::HostJobSeam;

/// The `rules.run_async` kind label on the durable job.
pub const RULE_RUN_KIND: &str = "rule-run";

/// Spawn the drive task for run `run_id` (already seeded as a `Running` job). Named function so
/// the future is its own type (the flows `drive_run_task` precedent).
pub fn spawn_worker(node: Arc<Node>, principal: Principal, ws: String, run_id: String) {
    tokio::spawn(drive_rule_run(node, principal, ws, run_id));
}

/// Drive one run to a settle. Every fault path writes a durable status — never a silent hang.
async fn drive_rule_run(node: Arc<Node>, principal: Principal, ws: String, run_id: String) {
    let control = node.rule_runs.insert(&ws, &run_id);

    let settle = run_once(&node, &principal, &ws, &run_id, control).await;

    node.rule_runs.remove(&ws, &run_id);

    if let Err(e) = settle {
        // A drive fault (payload decode, store write) — mark Failed durably, best-effort.
        let _ = lb_jobs::complete(&node.store, &ws, &run_id, JobStatus::Failed).await;
        tracing::warn!(target: "rules", run_id, error = %e, "rule-run drive failed");
    }
}

/// One attempt: load → fold → evaluate → settle. Returns Err only for drive-level faults (an
/// author error inside the body settles the job as `Failed` and returns Ok).
async fn run_once(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    run_id: &str,
    control: Arc<lb_rules::RunControl>,
) -> Result<(), String> {
    let job = lb_jobs::load(&node.store, ws, run_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("rule-run {run_id} not found"))?;
    let payload = RuleJobPayload::parse(&job)?;

    // Resolve the body: ad-hoc or by saved id (re-read on every attempt; the saved rule is the
    // durable truth, exactly as `rules.run` reads it).
    let (name, body) = match (&payload.body, &payload.rule_id) {
        (Some(b), _) => ("adhoc".to_string(), b.clone()),
        (None, Some(id)) => {
            let saved = super::super::get::rules_get(&node.store, principal, ws, id)
                .await
                .map_err(|e| format!("saved rule {id}: {e}"))?;
            (saved.name, saved.body)
        }
        (None, None) => return Err("rule-run payload has neither body nor rule_id".into()),
    };

    // The model seam — same resolution as `rules.run`, idempotency pinned to the run id so a
    // resumed attempt replays through the gateway turn cache instead of re-spending.
    let idem = format!("rules.run_async:{ws}:{run_id}");
    let model = super::super::resolve_rule_model(node, principal, ws, idem).await;

    let (data, allowed, messaging) = build_seams(node, principal, ws).await;
    let ai = Arc::new(HostAiSeam::new(model));
    let engine = RuleEngine::new(
        data,
        ai,
        messaging,
        job_rule_limits(),
        job_ai_limits(),
        job_max_writes(),
    )
    .with_route(payload.route);

    let job_seam = Arc::new(HostJobSeam::new(
        node.clone(),
        ws.to_string(),
        run_id.to_string(),
        tokio::runtime::Handle::current(),
        job.cursor,
    ));
    let state = fold_checkpoints(&job);
    let params = super::super::run::params_to_rhai(&payload.params);
    let rule = Rule {
        workspace: ws.to_string(),
        name,
        body,
        params: Vec::new(),
    };
    let now = payload.now;
    let opts = RunOptions {
        control: Some(control),
        job: Some(JobBinding {
            id: run_id.to_string(),
            seam: job_seam.clone(),
            state,
        }),
    };
    let allowed_arc = allowed;
    let (out, run) = tokio::task::spawn_blocking(move || {
        let mut run = RuleRun::new(rule.workspace.clone(), allowed_arc, params, now);
        let out = engine.run_with(&rule, &mut run, opts);
        (out, run)
    })
    .await
    .map_err(|e| format!("rule task panicked: {e}"))?;

    match out {
        Ok(output) => {
            // Route alerts once, on successful completion (a paused run routes on its finishing
            // attempt; deterministic ids make any replayed write an upsert).
            if payload.route {
                route_alerts(node, principal, ws, &run.findings, now)
                    .await
                    .map_err(|e| format!("alert routing failed: {e}"))?;
            }
            let result = RunResult {
                output,
                findings: run.findings,
                log: run.log,
                ms: 0,
                ai: run.ai_spend,
            };
            let _ = job_seam
                .record_reserved(
                    RESULT_KEY,
                    &serde_json::to_value(&result).unwrap_or(json!(null)),
                )
                .await;
            lb_jobs::complete(&node.store, ws, run_id, JobStatus::Done)
                .await
                .map_err(|e| e.to_string())?;
        }
        Err(RuleError::Paused) => {
            lb_jobs::suspend(&node.store, ws, run_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        Err(RuleError::Cancelled) => {
            lb_jobs::cancel(&node.store, ws, run_id)
                .await
                .map_err(|e| e.to_string())?;
        }
        Err(e) => {
            let _ = job_seam
                .record_reserved(ERROR_KEY, &json!(e.to_string()))
                .await;
            lb_jobs::complete(&node.store, ws, run_id, JobStatus::Failed)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
