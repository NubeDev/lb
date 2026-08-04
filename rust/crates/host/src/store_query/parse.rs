//! The **load-bearing** read-only gate of `store.query` (widget-builder Slice A): PARSE the SQL with
//! SurrealDB's own parser and allowlist by **statement kind** — never a substring/regex check (a
//! `LIKE '%delete%'` test is explicitly not acceptable; it both over- and under-matches). The query
//! must be exactly **one** statement, and that statement must be a read: `SELECT`, or an
//! introspection `INFO`/`SHOW`. Everything else is refused:
//!
//!   - mutations: `CREATE`, `UPDATE`, `UPSERT`, `DELETE`, `INSERT`, `RELATE`;
//!   - schema: `DEFINE`, `REMOVE`, `ALTER`, `REBUILD`;
//!   - control / transactions: `BEGIN`/`COMMIT`/`CANCEL`, `IF`, `FOR`, `LET`, `RETURN`, `THROW`,
//!     `SLEEP`, `KILL`, `LIVE`, `OPTION`, a bare value;
//!   - **namespace/database naming**: `USE` is refused outright — the workspace namespace is set
//!     host-side from the token (`query_ws`), never from the SQL, so a query can never escape its
//!     workspace (the wall, README §7). A `DEFINE NAMESPACE`/`DEFINE DATABASE` would already be
//!     refused as a `Define`.
//!
//! Mutation goes through the real typed write tools (`ingest.write`, `template.save`, …), never this
//! verb. This is the boundary; the visual builder (Slice C) is only convenience above it.

use surrealdb::sql::Statement;

use super::error::StoreQueryError;
use super::secret_wall::ensure_no_secret_read;

/// Which kind of read a validated statement is — the runner bounds a `SELECT` with a `LIMIT`/`TIMEOUT`
/// wrapper but runs `INFO`/`SHOW` (inherently single-row introspection) as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadKind {
    Select,
    Introspection,
}

/// Parse + allowlist `sql`. On success returns the single read statement's [`ReadKind`] (safe to run
/// inside the caller's workspace namespace). On failure a [`StoreQueryError::Rejected`] (a disallowed
/// kind / multi-statement) or [`StoreQueryError::Parse`] (invalid SurrealQL) — both safe to surface to
/// the SQL editor (they are author feedback, not an authorization signal).
/// It also carries the **secret-plane wall** ([`ensure_no_secret_read`]) on the same parsed statement
/// ([`StoreQueryError::SecretTable`]), so no caller can hold one gate and skip the other.
pub fn ensure_read_only(sql: &str) -> Result<ReadKind, StoreQueryError> {
    let query = surrealdb::syn::parse(sql).map_err(|e| StoreQueryError::Parse(e.to_string()))?;
    let statements = &query.0 .0;

    match statements.len() {
        0 => return Err(StoreQueryError::Rejected("empty query".into())),
        1 => {}
        n => {
            return Err(StoreQueryError::Rejected(format!(
                "only a single statement is allowed (got {n})"
            )))
        }
    }

    let stmt = &statements[0];

    let kind = match stmt {
        // Reads — allowed. A `SELECT` is the workhorse; `INFO`/`SHOW` are introspection a schema/
        // discovery view may need. None of these can mutate or name a namespace.
        Statement::Select(_) => ReadKind::Select,
        Statement::Info(_) | Statement::Show(_) => ReadKind::Introspection,

        // `USE` names a namespace/database — refused outright (the workspace wall is host-side).
        Statement::Use(_) => {
            return Err(StoreQueryError::Rejected(
                "USE (namespace/database selection) is not allowed — the workspace is fixed".into(),
            ))
        }

        // Everything else is a write, a schema change, control flow, a transaction boundary, or a
        // bare value — refused by kind. The message names the kind so the editor can explain it.
        other => {
            return Err(StoreQueryError::Rejected(format!(
                "only a single read (SELECT / INFO / SHOW) is allowed; '{}' is rejected",
                statement_kind(other)
            )))
        }
    };

    // The second, independent wall: a read of the right KIND may still touch the secret plane. It is
    // checked on the same parsed statement — see `secret_wall.rs` for what it can and cannot prove.
    ensure_no_secret_read(stmt)?;

    Ok(kind)
}

/// A short human label for a rejected statement kind (for the editor's error line).
fn statement_kind(s: &Statement) -> &'static str {
    match s {
        Statement::Create(_) => "CREATE",
        Statement::Update(_) => "UPDATE",
        Statement::Upsert(_) => "UPSERT",
        Statement::Delete(_) => "DELETE",
        Statement::Insert(_) => "INSERT",
        Statement::Relate(_) => "RELATE",
        Statement::Define(_) => "DEFINE",
        Statement::Remove(_) => "REMOVE",
        Statement::Alter(_) => "ALTER",
        Statement::Rebuild(_) => "REBUILD",
        Statement::Begin(_) | Statement::Commit(_) | Statement::Cancel(_) => "transaction control",
        Statement::Ifelse(_) | Statement::Foreach(_) => "control flow",
        Statement::Set(_) => "LET",
        Statement::Output(_) => "RETURN",
        Statement::Throw(_) => "THROW",
        Statement::Sleep(_) => "SLEEP",
        Statement::Kill(_) => "KILL",
        Statement::Live(_) => "LIVE",
        Statement::Option(_) => "OPTION",
        Statement::Use(_) => "USE",
        Statement::Value(_) => "a bare value",
        _ => "this statement",
    }
}
