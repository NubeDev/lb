//! `chains.get {id}` / `chains.list` / `chains.runs.get {run_id}` — workspace-scoped reads.
//! `chains.runs.get` rebuilds a run's live status + per-step results from the durable records (the
//! DAG-canvas read; also the snapshot a late `chains.watch` joiner gets).

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;
use serde_json::{json, Value};

use lb_rules::workflow::Chain;

use super::error::ChainsError;
use super::record::CHAIN_TABLE;
use super::run_store::{read_run, read_step};

/// Read one chain by id (skipping a tombstoned doc).
pub async fn chains_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<Chain, ChainsError> {
    authorize_store_read(principal, ws)?;
    let val = lb_store::read(store, ws, CHAIN_TABLE, id)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?
        .ok_or(ChainsError::NotFound)?;
    if val
        .get("deleted")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err(ChainsError::NotFound);
    }
    serde_json::from_value(val).map_err(|e| ChainsError::Internal(e.to_string()))
}

/// List chains in the workspace (non-deleted).
pub async fn chains_list(
    store: &Store,
    principal: &Principal,
    ws: &str,
) -> Result<Vec<Chain>, ChainsError> {
    authorize_store_read(principal, ws)?;
    let page = lb_store::scan(store, ws, CHAIN_TABLE, lb_store::MAX_SCAN_LIMIT, None)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?;
    let mut out = Vec::new();
    for row in page.rows {
        // Records written via `lb_store::write` carry a `{ data: ... }` envelope; `scan` returns the
        // whole record, so unwrap it before reading `deleted` / decoding the `Chain` — otherwise the
        // tombstone check never fires and every chain silently fails deser (the roster is always
        // empty). Mirrors the `scan_dashboards` envelope unwrap in `dashboard/store.rs`.
        let inner = match row.data {
            serde_json::Value::Object(mut o) => o.remove("data").unwrap_or(serde_json::Value::Null),
            other => other,
        };
        if inner
            .get("deleted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            continue;
        }
        if let Ok(c) = serde_json::from_value::<Chain>(inner) {
            out.push(c);
        }
    }
    Ok(out)
}

/// Read a run's live status + per-step results (rebuilt from the durable records). The snapshot a late
/// `chains.watch` joiner gets, and the DAG-canvas poll.
pub async fn chains_run_get(
    store: &Store,
    principal: &Principal,
    ws: &str,
    chain: &Chain,
    run_id: &str,
) -> Result<Value, ChainsError> {
    authorize_store_read(principal, ws)?;
    let run = read_run(store, ws, run_id)
        .await
        .map_err(ChainsError::Internal)?
        .ok_or(ChainsError::NotFound)?;
    let mut steps = Vec::new();
    for step in &chain.steps {
        if let Some(rec) = read_step(store, ws, run_id, &step.id)
            .await
            .map_err(ChainsError::Internal)?
        {
            steps.push(json!({
                "id": rec.step_id,
                "claim": format!("{:?}", rec.claim).to_lowercase(),
                "outcome": rec.outcome,
                "output": rec.output,
                "findings": rec.findings,
                "error": rec.error,
            }));
        }
    }
    Ok(
        json!({ "run_id": run.run_id, "chain_id": run.chain_id, "status": run.status, "steps": steps }),
    )
}

fn authorize_store_read(principal: &Principal, ws: &str) -> Result<(), ChainsError> {
    let req = Request::new(ws, Surface::Store, "chain", Action::Read);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(ChainsError::Denied),
    }
}
