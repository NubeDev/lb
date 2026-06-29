//! Target resolution — parse a saved query's `target` into the engine it dispatches to, and expose
//! the **no-widening** map: the underlying capability `query.run` must additionally hold for that
//! target (`mcp:store.query:call` for platform, `mcp:federation.query:call` for a datasource). The
//! target is host-resolved in the caller's workspace only — a caller cannot name a cross-tenant
//! datasource (the wall is at resolution, query scope "Ad-hoc cross-tenant targets").

use lb_prql::{dialect_for_kind, dialect_for_target, Dialect, PrqlError};

use super::error::QueryError;

/// The parsed target of a saved query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryTarget {
    /// Platform data — dispatches to `store.query` (SurrealDB, native). Compiles PRQL to `Generic`.
    Platform,
    /// An external datasource registered in the caller's workspace — dispatches to `federation.query`.
    /// Carries the datasource name; the host resolves its `kind` for the dialect.
    Datasource(String),
}

/// The `target` string prefix naming an external datasource.
pub const DATASOURCE_PREFIX: &str = "datasource:";

impl QueryTarget {
    /// Parse a `target` string (`"platform"` | `"datasource:<name>"`). Anything else is bad input.
    pub fn parse(target: &str) -> Result<Self, QueryError> {
        if target == "platform" {
            return Ok(QueryTarget::Platform);
        }
        if let Some(name) = target.strip_prefix(DATASOURCE_PREFIX) {
            if name.is_empty() {
                return Err(QueryError::BadInput("empty datasource name".into()));
            }
            return Ok(QueryTarget::Datasource(name.to_string()));
        }
        Err(QueryError::BadInput(format!(
            "unknown target `{target}` — expected `platform` or `datasource:<name>`"
        )))
    }

    /// The qualified tool `query.run` re-dispatches to for this target — the underlying cap the
    /// caller must additionally hold (rule 5, no-widening).
    pub fn underlying_tool(&self) -> &'static str {
        match self {
            QueryTarget::Platform => "store.query",
            QueryTarget::Datasource(_) => "federation.query",
        }
    }

    /// The PRQL dialect for a platform target. (A datasource's dialect needs its `kind`; see
    /// [`dialect_for_datasource`].)
    pub fn platform_dialect() -> Result<Dialect, PrqlError> {
        dialect_for_target("platform")
    }
}

/// The PRQL dialect for a datasource of `kind` (resolved from the registered record's kind).
pub fn dialect_for_datasource(kind: &str) -> Result<Dialect, QueryError> {
    dialect_for_kind(kind).map_err(|e| QueryError::BadInput(e.to_string()))
}
