//! `query.run {id}` (or `{lang, text, target}` for an unsaved one-shot) → `{columns, rows}` (query
//! scope). The headline verb. The host:
//!   1. authorizes `mcp:query.run:call` (workspace-first);
//!   2. resolves the record by `id` IN THE CALLER'S WORKSPACE, or takes the inline one-shot;
//!   3. compiles `text` for the target's dialect (or carries `raw` verbatim) — see [`materialize`];
//!   4. checks declared vs provided params (missing/extra is a typed error; `$var` binds through the
//!      engine's real param path — `store.query` `vars` — never string interpolation);
//!   5. **composes, never widens (rule 5):** re-authorizes the target's underlying cap
//!      (`mcp:store.query:call` for platform, `mcp:federation.query:call` for a datasource). Holding
//!      `query.run` ALONE is denied — the headline no-widening deny;
//!   6. dispatches to `store.query` (platform) or `federation.query` (datasource), returning
//!      `{columns, rows}` in the same shape both engines use.
//!
//! SurrealDB stays the authority (rule 2): the platform target reads native data through the existing
//! `store.query` read-only gate; a datasource target only reaches a registered EXTERNAL source.

use std::sync::Arc;

use lb_auth::Principal;
use lb_supervisor::OsLauncher;
use serde_json::{json, Value};

use super::authorize::authorize;
use super::error::QueryError;
use super::materialize::{materialize, validate_params};
use super::record::resolve;
use super::save::validate_lang;
use super::target::QueryTarget;
use crate::boot::Node;
use crate::federation::federation_query;
use crate::store_query::{store_query_run, QueryResult};

/// The source for a run: a saved query by `id`, or an inline unsaved one-shot.
pub enum RunSource {
    /// Run a saved `query:{ws}:{id}` record.
    ById(String),
    /// Run an ad-hoc, unsaved query (the editor's pre-save path / a channel one-shot).
    Inline {
        lang: String,
        text: String,
        target: String,
        params: Vec<String>,
    },
}

/// Run a saved (by id) or inline query and return `{columns, rows}`. `vars` bind `$var` through the
/// engine's real param path.
pub async fn query_run(
    node: &Arc<Node>,
    caller: &Principal,
    ws: &str,
    src: RunSource,
    vars: Vec<(String, Value)>,
    ts: u64,
) -> Result<Value, QueryError> {
    authorize(caller, ws, "query.run")?;

    // Resolve the record (or take the inline one-shot). A cross-tenant id resolves to NotFound here.
    let (lang, text, target, params) = match src {
        RunSource::ById(id) => {
            let q = resolve(&node.store, ws, &id)
                .await?
                .ok_or(QueryError::NotFound)?;
            (q.lang, q.text, q.target, q.params)
        }
        RunSource::Inline {
            lang,
            text,
            target,
            params,
        } => (lang, text, target, params),
    };

    validate_lang(&lang)?;
    let parsed = QueryTarget::parse(&target)?;
    validate_params(&target, &params)?;

    // NO-WIDENING (rule 5): the caller must ALSO hold the target's underlying cap. This is checked
    // BEFORE compile/resolution so the headline deny bites first — a caller with `query.run` but
    // without the target cap is denied even if the datasource is absent or the PRQL is malformed.
    // Opaque, like any capability miss.
    authorize(caller, ws, parsed.underlying_tool())?;

    // Missing/extra param is a typed author error (injection-safe: vars bind by name, never spliced).
    check_params(&params, &vars)?;

    // Compile (or carry raw) to the target's SQL.
    let sql = materialize(node, ws, &lang, &text, &target).await?;

    // Dispatch to the engine that already owns the wall. Each re-authorizes its own cap (defense in
    // depth) — `query.run` composes them, it does not widen them.
    match parsed {
        QueryTarget::Platform => {
            let result: QueryResult = store_query_run(&node.store, caller, ws, &sql, vars)
                .await
                .map_err(platform_err)?;
            Ok(json!({ "columns": result.columns, "rows": result.rows }))
        }
        QueryTarget::Datasource(name) => {
            // The federation path has no bind-param path in v1 (validate_params rejected params above),
            // so vars are necessarily empty here — no interpolation, ever.
            let launcher = OsLauncher;
            federation_query(node, &launcher, caller, ws, &name, &sql, ts)
                .await
                .map_err(|e| match e {
                    crate::federation::FederationError::NotFound => {
                        QueryError::BadInput("no such datasource".into())
                    }
                    other => QueryError::BadInput(other.to_string()),
                })
        }
    }
}

/// Map the platform gate's outcome. `Denied` stays opaque (a missing `store.query` cap — already
/// pre-checked, so this is defense in depth); a `Rejected`/`Parse` reason is AUTHOR feedback (an
/// out-of-subset PRQL, or a non-SELECT `raw`) surfaced as `BadInput`.
fn platform_err(e: crate::store_query::StoreQueryError) -> QueryError {
    match e {
        crate::store_query::StoreQueryError::Denied => QueryError::Denied,
        crate::store_query::StoreQueryError::Rejected(m) => {
            QueryError::BadInput(format!("rejected by store.query gate: {m}"))
        }
        crate::store_query::StoreQueryError::Parse(m) => {
            QueryError::BadInput(format!("parse error: {m}"))
        }
        crate::store_query::StoreQueryError::Store(s) => QueryError::Store(s),
    }
}

/// A declared param missing from `vars`, or a `vars` entry not declared, is a typed error. Vars bind
/// by name through the engine's param path — this check is what makes that binding total.
fn check_params(declared: &[String], vars: &[(String, Value)]) -> Result<(), QueryError> {
    let provided: std::collections::HashSet<&str> = vars.iter().map(|(k, _)| k.as_str()).collect();
    for p in declared {
        if !provided.contains(p.as_str()) {
            return Err(QueryError::BadInput(format!("missing param `{p}`")));
        }
    }
    let declared_set: std::collections::HashSet<&str> =
        declared.iter().map(|s| s.as_str()).collect();
    for (k, _) in vars {
        if !declared_set.contains(k.as_str()) {
            return Err(QueryError::BadInput(format!("undeclared param `{k}`")));
        }
    }
    Ok(())
}
