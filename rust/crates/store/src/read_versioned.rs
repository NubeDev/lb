//! Read a value together with its store-managed `rev` — the unit the undo journal's conditional
//! restore predicate works on (`docs/scope/undo/undo-scope.md`).
//!
//! Distinct from the plain [`read`](crate::read), which returns only the host `data` and is the
//! API every existing caller uses (unchanged). This verb additionally surfaces the `rev` so a
//! caller can ask "is this record still the revision I expect?" before overwriting it — and it
//! reports **absence** as a first-class state ([`Versioned::absent`]), because a *create* undo
//! must assert "still absent" and a *delete* undo must restore from absence.

use serde_json::Value;

use crate::open::{Store, StoreError};
use crate::record::{Record, Versioned};

/// Fetch `table:id` from workspace `ws` with its `rev`. An absent record returns
/// [`Versioned::absent`] (value `None`, rev `0`) — never an error.
pub async fn read_versioned(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
) -> Result<Versioned, StoreError> {
    let mut response = store
        .query_ws_retrying(
            ws,
            "SELECT data, rev FROM ONLY type::record($tb, $id)",
            vec![
                ("tb".into(), Value::String(table.to_string())),
                ("id".into(), Value::String(id.to_string())),
            ],
        )
        .await?;
    let record: Option<Record> = response
        .take(0)
        .map_err(|e| StoreError::Decode(e.to_string()))?;
    Ok(match record {
        Some(r) => Versioned {
            value: Some(r.data),
            rev: r.rev,
        },
        None => Versioned::absent(),
    })
}
