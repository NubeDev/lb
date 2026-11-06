//! `schedule.delete`. Nodes referencing a deleted schedule then evaluate inactive rather than failing
//! the run: one deleted record must not wedge every flow that pointed at it.

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize_write;
use super::SCHEDULE_TABLE;
use crate::flows::error::FlowsError;

pub async fn schedule_delete(
    store: &Store,
    principal: &Principal,
    ws: &str,
    id: &str,
) -> Result<(), FlowsError> {
    authorize_write(principal, ws)?;
    lb_store::delete(store, ws, SCHEDULE_TABLE, id)
        .await
        .map_err(|e| FlowsError::Internal(e.to_string()))
}
