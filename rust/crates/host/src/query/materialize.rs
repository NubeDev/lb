//! Compile a saved/inline query's `text` to the SQL its target engine runs (query scope). For
//! `lang:"prql"`, compile via `lb-prql` for the target's dialect (platform → `Generic`; a datasource →
//! its `kind`'s dialect, resolved in the caller's workspace). For `lang:"raw"`, carry the text
//! verbatim — raw SurrealQL for platform, raw SQL for a datasource (the escape hatch). Pure of data
//! access: used by both `query.compile` (dry-run) and `query.run` (dispatch), so the two agree.

use lb_prql::compile as prql_compile;

use super::error::QueryError;
use super::target::{dialect_for_datasource, QueryTarget, DATASOURCE_PREFIX};
use crate::boot::Node;
use crate::federation::resolve_datasource;

/// Compile `text` (in `lang`) to SQL for `target` in `ws`. Resolves a datasource target's `kind` from
/// the registered record in the caller's workspace (a cross-tenant datasource resolves to nothing →
/// `NotFound`). `raw` is returned verbatim.
pub async fn materialize(
    node: &Node,
    ws: &str,
    lang: &str,
    text: &str,
    target: &str,
) -> Result<String, QueryError> {
    if lang == "raw" {
        return Ok(text.to_string());
    }
    // lang == "prql" (validated upstream).
    let parsed = QueryTarget::parse(target)?;
    let dialect =
        match parsed {
            QueryTarget::Platform => {
                QueryTarget::platform_dialect().map_err(|e| QueryError::BadInput(e.to_string()))?
            }
            QueryTarget::Datasource(name) => {
                let ds = resolve_datasource(&node.store, ws, &name).await?.ok_or(
                    QueryError::BadInput(format!("no such datasource for target `{target}`")),
                )?;
                dialect_for_datasource(&ds.kind)?
            }
        };
    prql_compile(text, dialect).map_err(|e| QueryError::Compile(e.to_string()))
}

/// The declared params a `target` may bind. Platform binds `$var` through `store.query` (full Phase-1
/// support). A datasource target's `federation.query` sidecar does not yet expose a bind-param path —
/// a parameterized datasource query is a typed error (loud, never string interpolation) until the
/// federation sidecar grows one. See query-scope "Params".
pub fn validate_params(target: &str, params: &[String]) -> Result<(), QueryError> {
    if !params.is_empty() && target.starts_with(DATASOURCE_PREFIX) {
        return Err(QueryError::BadInput(
            "parameterized datasource queries are not supported in v1 \
             (federation.query has no bind-param path yet); use `target:\"platform\"`"
                .into(),
        ));
    }
    Ok(())
}
