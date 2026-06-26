//! Load a job record from the workspace namespace — the resume read (jobs scope, agent scope).
//!
//! Because the namespace is selected from `ws`, a load for workspace A returns `None` for a job
//! that lives in workspace B's namespace — even with the same `job:{id}`. That is the
//! workspace-isolation guarantee at the store layer (README §7): a ws-B resume can never read a
//! ws-A session. Raw verb — the agent service checks caps first.

use lb_store::{read, Store, StoreError};

use super::model::Job;
use super::TABLE;

/// Fetch `job:{id}` from workspace `ws`. `None` if absent in *this* namespace.
pub async fn load(store: &Store, ws: &str, id: &str) -> Result<Option<Job>, StoreError> {
    match read(store, ws, TABLE, id).await? {
        Some(value) => {
            let job =
                serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
            Ok(Some(job))
        }
        None => Ok(None),
    }
}
