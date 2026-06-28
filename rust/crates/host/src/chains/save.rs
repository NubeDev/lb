//! `chains.save` — create/update a chain `chain:{ws}:{id}`, **validating the DAG up front** (a bad DAG
//! is a deny-equivalent before any run). `chains.delete` — tombstone (idempotent). Gated at the bridge
//! (`mcp:chains.save`/`mcp:chains.delete`); here the store-write surface + DAG validation.

use lb_auth::Principal;
use lb_caps::{check, Action, Decision, Request, Surface};
use lb_store::Store;

use lb_rules::workflow::Chain;

use super::error::ChainsError;
use super::record::CHAIN_TABLE;
use crate::rules::max_chain_steps;

/// Persist a chain after validating its DAG (cycle/dangling/dup/self-edge/size). Returns the id.
pub async fn chains_save(
    store: &Store,
    principal: &Principal,
    ws: &str,
    chain: &Chain,
) -> Result<String, ChainsError> {
    authorize_store_write(principal, ws)?;
    chain.validate(max_chain_steps())?; // rejected before any run
    let value = serde_json::to_value(chain).map_err(|e| ChainsError::Internal(e.to_string()))?;
    lb_store::write(store, ws, CHAIN_TABLE, &chain.id, &value)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?;
    Ok(chain.id.clone())
}

/// Delete a chain (hard delete — a chain has no sync-merge concern like a live record stream; an
/// absent chain is a no-op).
pub async fn chains_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), ChainsError> {
    authorize_store_write(principal, ws)?;
    // Tombstone-style: read, if present rewrite as empty-steps marker is wrong; simplest idempotent
    // delete is an overwrite to a deleted marker. We keep it simple: write a tombstone chain doc.
    let existing = lb_store::read(store, ws, CHAIN_TABLE, id)
        .await
        .map_err(|e| ChainsError::Internal(e.to_string()))?;
    if existing.is_none() {
        return Ok(());
    }
    lb_store::write(
        store,
        ws,
        CHAIN_TABLE,
        id,
        &serde_json::json!({ "id": id, "deleted": true }),
    )
    .await
    .map_err(|e| ChainsError::Internal(e.to_string()))?;
    Ok(())
}

fn authorize_store_write(principal: &Principal, ws: &str) -> Result<(), ChainsError> {
    let req = Request::new(ws, Surface::Store, "chain", Action::Write);
    match check(principal, &req) {
        Decision::Allowed => Ok(()),
        Decision::Denied(_) => Err(ChainsError::Denied),
    }
}
