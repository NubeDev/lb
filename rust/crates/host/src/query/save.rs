//! `query.save {id, name, description?, lang, text, target, params?}` — create/update a saved query
//! `query:{ws}:{id}` (query scope CRUD). Gated `mcp:query.save:call` (workspace-first) at the bridge.
//! `id` is the stable kebab-case slug (unique per ws); `name` is the editable display label (defaults
//! to `id`). Idempotent upsert on `id` — the save-and-re-edit ask (overwrite in place, like a
//! datasource; no revision history in v1). The text is NOT executed at save (a save is not a run).

use lb_auth::Principal;
use lb_store::Store;

use super::authorize::authorize;
use super::error::QueryError;
use super::record::{put, SavedQuery};
use super::target::QueryTarget;

/// Validate the lang/target and persist a saved query. Returns the id.
#[allow(clippy::too_many_arguments)]
pub async fn query_save(
    store: &Store,
    caller: &Principal,
    ws: &str,
    id: &str,
    name: &str,
    description: Option<&str>,
    lang: &str,
    text: &str,
    target: &str,
    params: Vec<String>,
    ts: u64,
) -> Result<String, QueryError> {
    authorize(caller, ws, "query.save")?;
    validate_lang(lang)?;
    QueryTarget::parse(target)?;
    if id.trim().is_empty() {
        return Err(QueryError::BadInput("id must not be empty".into()));
    }
    let display = if name.trim().is_empty() { id } else { name };
    let q = SavedQuery::new(
        id,
        display,
        description.unwrap_or(""),
        lang,
        text,
        target,
        params,
        ts,
    );
    put(store, ws, &q).await?;
    Ok(id.to_string())
}

/// The two authoring languages. `prql` compiles to the target's dialect; `raw` carries target-native
/// text verbatim (raw SurrealQL for platform, raw SQL for a datasource) — the escape hatch.
pub(crate) fn validate_lang(lang: &str) -> Result<(), QueryError> {
    match lang {
        "prql" | "raw" => Ok(()),
        other => Err(QueryError::BadInput(format!(
            "unknown lang `{other}` — expected `prql` or `raw`"
        ))),
    }
}
