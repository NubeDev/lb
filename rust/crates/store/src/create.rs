//! Create a JSON value at `<table>:<id>` within a workspace's namespace — the **first-write**
//! verb, the conditional counterpart to [`write`](crate::write)'s upsert.
//!
//! Unlike `write` (an UPSERT — last-writer-wins, used where re-applying the same record must be a
//! no-op), `create` uses SurrealDB `CREATE`, which **errors if the record already exists**. That
//! "first write binds, a second is rejected" semantic is exactly what an agent's **Ask decision**
//! needs (agent-run scope Part 2 first-settle): once a tool call is decided and acted on, a later
//! decision must NOT flip it. Two `agent.decide` calls on the same `{job, tool_call}` → the first
//! `create` binds, the second hits [`StoreError::Conflict`] — never a silent upsert.
//!
//! The namespace is selected from `ws` first, so a create can only land in its own workspace
//! (README §7). The host JSON is wrapped under `data` (the same envelope as `write`/`read`), so the
//! read path is identical. Raw verb — `caps::check` runs before this; it is not the auth point.

use serde_json::Value;

use crate::open::{Store, StoreError};

/// Create `value` at `table:id` in workspace `ws`. Returns [`StoreError::Conflict`] if a record
/// already exists at that id (the first-write guarantee) — every other failure is a `Backend` error.
pub async fn create(
    store: &Store,
    ws: &str,
    table: &str,
    id: &str,
    value: &Value,
) -> Result<(), StoreError> {
    let db = store.use_ws(ws).await?;
    let result = db
        .query("CREATE type::thing($tb, $id) CONTENT { data: $data }")
        .bind(("tb", table.to_string()))
        .bind(("id", id.to_string()))
        .bind(("data", value.clone()))
        .await?
        .check();
    match result {
        Ok(_) => Ok(()),
        // SurrealDB reports a duplicate-id CREATE as an "already exists" record error. We translate
        // it to the typed `Conflict` so a first-settle caller can branch on it cleanly, rather than
        // string-matching a backend message at the call site.
        Err(e) if is_already_exists(&e) => Err(StoreError::Conflict),
        Err(e) => Err(e.into()),
    }
}

/// Whether a SurrealDB error is the "record already exists" outcome of a duplicate-id `CREATE`.
/// SurrealDB does not expose a stable typed variant for this across versions, so we match on the
/// message — pinned to the phrase SurrealDB uses (`already exists`). Kept in one place so a version
/// bump that changes the wording is a one-line fix, not a hunt across call sites.
fn is_already_exists(e: &surrealdb::Error) -> bool {
    e.to_string().contains("already exists")
}
