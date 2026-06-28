//! The durable run-store over SurrealDB — the backend for rubix-cube's `WorkflowRunStore` trait shape,
//! ported to our store (rule-chains-scope: "keep the trait shape, implement over our store"). The CAS
//! claim (`Pending|Enqueued → Running`) is the idempotency guard under redelivery — a lost claim
//! no-ops, so a duplicate step redelivery never double-runs a rule. `ready_dependents`/`skip_subtree`/
//! `finalize_if_complete` mirror the in-memory logic, persisted per step so concurrent writes don't
//! contend and a restart resumes from the recorded state.

use lb_store::Store;
use serde_json::Value;

use lb_rules::workflow::{Chain, ChainResult, ChainStatus, Outcome, RunContext, StepRecord};
use lb_rules::RuleOutput;

use super::record::{
    step_record_id, ChainRunRecord, ClaimState, StepStateRecord, CHAIN_RUN_TABLE, CHAIN_STEP_TABLE,
};

/// Seed a run: insert the lifecycle record (Pending) + a per-step state row (claim from in-degree).
pub async fn create_run(
    store: &Store,
    ws: &str,
    run_id: &str,
    chain: &Chain,
) -> Result<(), String> {
    let run = ChainRunRecord {
        run_id: run_id.to_string(),
        chain_id: chain.id.clone(),
        status: "pending".to_string(),
    };
    lb_store::write(
        store,
        ws,
        CHAIN_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;

    let indeg = chain.indegrees();
    for step in &chain.steps {
        let d = indeg[&step.id];
        let claim = if d == 0 {
            ClaimState::Enqueued
        } else {
            ClaimState::Pending
        };
        let rec = StepStateRecord {
            run_id: run_id.to_string(),
            step_id: step.id.clone(),
            claim,
            indegree: d,
            outcome: String::new(),
            output: Value::Null,
            findings: Value::Null,
            error: None,
            attempts: 0,
            ms: 0,
        };
        write_step(store, ws, &rec).await?;
    }
    Ok(())
}

/// CAS claim a step: `Pending|Enqueued → Running`. Returns true if THIS call won the claim (false if
/// already Running/Done — the redelivery no-op).
pub async fn claim_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    step_id: &str,
) -> Result<bool, String> {
    let Some(mut rec) = read_step(store, ws, run_id, step_id).await? else {
        return Ok(false);
    };
    match rec.claim {
        ClaimState::Pending | ClaimState::Enqueued => {
            rec.claim = ClaimState::Running;
            write_step(store, ws, &rec).await?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Record a step's terminal outcome (idempotent — overwrites the row's result fields).
pub async fn record_outcome(
    store: &Store,
    ws: &str,
    run_id: &str,
    step_id: &str,
    rec: &StepRecord,
) -> Result<(), String> {
    let Some(mut state) = read_step(store, ws, run_id, step_id).await? else {
        return Ok(());
    };
    state.claim = ClaimState::Done;
    state.attempts = rec.attempts;
    state.ms = rec.ms;
    match &rec.outcome {
        Outcome::Ok(out, findings) => {
            state.outcome = "ok".into();
            state.output = output_json(out);
            state.findings = serde_json::to_value(findings).unwrap_or(Value::Null);
        }
        Outcome::Err(e) => {
            state.outcome = "err".into();
            state.error = Some(e.clone());
        }
        Outcome::Skipped => {
            state.outcome = "skipped".into();
        }
    }
    write_step(store, ws, &state).await
}

/// Decrement dependents' in-degree; return those that reached 0 (marked Enqueued). Ported from
/// rubix-cube's `ready_dependents`.
pub async fn ready_dependents(
    store: &Store,
    ws: &str,
    chain: &Chain,
    run_id: &str,
    finished: &str,
) -> Result<Vec<String>, String> {
    let dependents = chain.dependents();
    let deps = dependents.get(finished).cloned().unwrap_or_default();
    let mut ready = Vec::new();
    for dep in deps {
        let Some(mut rec) = read_step(store, ws, run_id, &dep).await? else {
            continue;
        };
        if rec.claim != ClaimState::Pending {
            // already enqueued/running/done via another path — still decrement so the count is right
            rec.indegree = rec.indegree.saturating_sub(1);
            write_step(store, ws, &rec).await?;
            continue;
        }
        rec.indegree = rec.indegree.saturating_sub(1);
        if rec.indegree == 0 {
            rec.claim = ClaimState::Enqueued;
            ready.push(dep);
        }
        write_step(store, ws, &rec).await?;
    }
    Ok(ready)
}

/// Mark the transitive subtree below a failed step as Skipped (Halt policy). Ported from rubix-cube's
/// `skip_subtree` (BFS over dependents).
pub async fn skip_subtree(
    store: &Store,
    ws: &str,
    chain: &Chain,
    run_id: &str,
    failed: &str,
) -> Result<(), String> {
    let dependents = chain.dependents();
    let mut queue: std::collections::VecDeque<String> =
        dependents.get(failed).cloned().unwrap_or_default().into();
    let mut seen = std::collections::HashSet::new();
    while let Some(id) = queue.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        if let Some(mut rec) = read_step(store, ws, run_id, &id).await? {
            if matches!(rec.claim, ClaimState::Pending | ClaimState::Enqueued) {
                rec.claim = ClaimState::Done;
                rec.outcome = "skipped".into();
                write_step(store, ws, &rec).await?;
            }
        }
        if let Some(next) = dependents.get(&id) {
            for n in next {
                queue.push_back(n.clone());
            }
        }
    }
    Ok(())
}

/// If every step is Done, collapse the run into a `ChainResult` and write the terminal status.
pub async fn finalize_if_complete(
    store: &Store,
    ws: &str,
    chain: &Chain,
    run_id: &str,
) -> Result<Option<ChainResult>, String> {
    let mut ctx = RunContext::new(chain.params.clone());
    for step in &chain.steps {
        let Some(rec) = read_step(store, ws, run_id, &step.id).await? else {
            return Ok(None);
        };
        if rec.claim != ClaimState::Done {
            return Ok(None);
        }
        ctx.record(step.id.clone(), to_step_record(&rec));
    }
    let result = ctx.to_result();
    let status = match result.status {
        ChainStatus::Success => "success",
        ChainStatus::PartialFailure => "partialFailure",
        ChainStatus::Failed => "failed",
    };
    let mut run = read_run(store, ws, run_id)
        .await?
        .ok_or_else(|| "run record missing".to_string())?;
    run.status = status.to_string();
    lb_store::write(
        store,
        ws,
        CHAIN_RUN_TABLE,
        run_id,
        &serde_json::to_value(&run).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())?;
    Ok(Some(result))
}

/// Resolve a step's `with` bindings against the recorded upstream outputs + the chain params.
pub async fn resolve_bindings(
    store: &Store,
    ws: &str,
    chain: &Chain,
    run_id: &str,
    with: &serde_json::Map<String, Value>,
) -> Result<rhai::Map, String> {
    let mut ctx = RunContext::new(chain.params.clone());
    for step in &chain.steps {
        if let Some(rec) = read_step(store, ws, run_id, &step.id).await? {
            if rec.claim == ClaimState::Done {
                ctx.record(step.id.clone(), to_step_record(&rec));
            }
        }
    }
    ctx.resolve_bindings(with)
}

// ---- the read/write helpers + conversions ----

async fn write_step(store: &Store, ws: &str, rec: &StepStateRecord) -> Result<(), String> {
    let id = step_record_id(&rec.run_id, &rec.step_id);
    lb_store::write(
        store,
        ws,
        CHAIN_STEP_TABLE,
        &id,
        &serde_json::to_value(rec).map_err(|e| e.to_string())?,
    )
    .await
    .map_err(|e| e.to_string())
}

pub async fn read_step(
    store: &Store,
    ws: &str,
    run_id: &str,
    step_id: &str,
) -> Result<Option<StepStateRecord>, String> {
    let id = step_record_id(run_id, step_id);
    match lb_store::read(store, ws, CHAIN_STEP_TABLE, &id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn read_run(
    store: &Store,
    ws: &str,
    run_id: &str,
) -> Result<Option<ChainRunRecord>, String> {
    match lb_store::read(store, ws, CHAIN_RUN_TABLE, run_id).await {
        Ok(Some(v)) => serde_json::from_value(v)
            .map(Some)
            .map_err(|e| e.to_string()),
        Ok(None) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn output_json(out: &RuleOutput) -> Value {
    match out {
        RuleOutput::Scalar(v) => v.clone(),
        RuleOutput::Grid(g) => serde_json::json!({ "columns": g.columns, "rows": g.rows }),
        RuleOutput::Findings | RuleOutput::Nothing => Value::Null,
    }
}

/// Rebuild a `StepRecord` from the persisted state (for the result + binding resolution).
fn to_step_record(rec: &StepStateRecord) -> StepRecord {
    let outcome = match rec.outcome.as_str() {
        "ok" => {
            let findings = serde_json::from_value(rec.findings.clone()).unwrap_or_default();
            Outcome::Ok(RuleOutput::Scalar(rec.output.clone()), findings)
        }
        "err" => Outcome::Err(rec.error.clone().unwrap_or_default()),
        _ => Outcome::Skipped,
    };
    StepRecord {
        outcome,
        attempts: rec.attempts,
        ms: rec.ms,
    }
}
