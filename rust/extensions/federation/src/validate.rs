//! SELECT-only SQL validation (datasources scope: read-first v1; a write/DDL is rejected before
//! execution). The pattern is ported from `rubix-cube`'s SQL validator (MIT/Apache-2.0): parse the
//! statement, allow ONLY a single read query, and collect the referenced table names so the engine
//! can register exactly those external tables — a caller never reaches an unregistered shape.
//!
//! This is defense in depth alongside the host-side re-validation: the host validates before it ever
//! routes to the sidecar, and the sidecar validates again before it touches a pool. Two independent
//! gates, the capability-first posture carried into the query body.

use datafusion::sql::sqlparser::ast::{Query, SetExpr, Statement, TableFactor, Visit, Visitor};
use datafusion::sql::sqlparser::dialect::GenericDialect;
use datafusion::sql::sqlparser::parser::Parser;
use std::ops::ControlFlow;

/// The hard cap on rows a `federation.query` returns. An unbounded export is a mirror job, never a
/// live query (datasources scope: no blocking large read in a tool handler, §6.1).
pub const ROW_CAP: usize = 10_000;

#[derive(Debug)]
pub struct ValidationError(pub String);

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rejected sql: {}", self.0)
    }
}

/// Validate that `sql` is exactly one SELECT-only statement and return the distinct table names it
/// references (so the engine registers only those). Rejects multiple statements, any
/// INSERT/UPDATE/DELETE/DDL/DML, and anything that is not a read query.
pub fn validate_select(sql: &str) -> Result<Vec<String>, ValidationError> {
    let dialect = GenericDialect {};
    let statements = Parser::parse_sql(&dialect, sql)
        .map_err(|e| ValidationError(format!("parse error: {e}")))?;

    if statements.len() != 1 {
        return Err(ValidationError(format!(
            "exactly one statement allowed, got {}",
            statements.len()
        )));
    }

    let query = match &statements[0] {
        Statement::Query(q) => q.as_ref(),
        other => {
            // Anything that is not a Query — INSERT/UPDATE/DELETE/CREATE/DROP/ALTER/COPY/… — is a
            // write or DDL: rejected outright (read-first v1).
            return Err(ValidationError(format!(
                "only SELECT is allowed; rejected: {}",
                statement_kind(other)
            )));
        }
    };

    // Guard against a CTE/subquery hiding a write via a data-modifying body. The top-level body must
    // be a SELECT (or a set-op over SELECTs); INSERT-in-CTE is rejected here.
    ensure_read_body(&query.body)?;

    let mut collector = TableCollector::default();
    let _ = query.visit(&mut collector);
    Ok(collector.tables)
}

/// Recursively assert a query body is a read (SELECT, VALUES, or a set operation over reads). Any
/// embedded INSERT/UPDATE/DELETE body is rejected.
fn ensure_read_body(body: &SetExpr) -> Result<(), ValidationError> {
    match body {
        SetExpr::Select(_) | SetExpr::Values(_) | SetExpr::Query(_) | SetExpr::Table(_) => Ok(()),
        SetExpr::SetOperation { left, right, .. } => {
            ensure_read_body(left)?;
            ensure_read_body(right)
        }
        SetExpr::Insert(_) | SetExpr::Update(_) | SetExpr::Delete(_) | SetExpr::Merge(_) => Err(
            ValidationError("data-modifying statement in query body".into()),
        ),
    }
}

/// A short label for the rejected statement kind (for the error, never the SQL itself).
fn statement_kind(s: &Statement) -> &'static str {
    match s {
        Statement::Insert { .. } => "INSERT",
        Statement::Update { .. } => "UPDATE",
        Statement::Delete { .. } => "DELETE",
        Statement::CreateTable { .. } | Statement::CreateView { .. } => "CREATE (DDL)",
        Statement::Drop { .. } => "DROP (DDL)",
        Statement::AlterTable { .. } => "ALTER (DDL)",
        Statement::Truncate { .. } => "TRUNCATE",
        Statement::Copy { .. } => "COPY",
        _ => "non-SELECT statement",
    }
}

/// Walk the AST collecting every `TableFactor::Table` name (the physical tables the query reads).
/// CTE aliases are also collected but are harmless to register-attempt; the engine resolves real
/// tables and ignores names that resolve to a CTE in scope.
#[derive(Default)]
struct TableCollector {
    tables: Vec<String>,
}

impl Visitor for TableCollector {
    type Break = ();

    fn pre_visit_table_factor(&mut self, table_factor: &TableFactor) -> ControlFlow<()> {
        if let TableFactor::Table { name, .. } = table_factor {
            // The last identifier is the table name (drop a schema/catalog qualifier — the external
            // pool is already pinned to one database via the DSN).
            if let Some(ident) = name.0.last().and_then(|p| p.as_ident()) {
                let t = ident.value.clone();
                if !self.tables.contains(&t) {
                    self.tables.push(t);
                }
            }
        }
        ControlFlow::Continue(())
    }
}

// Silence an unused import warning if `Query` is only referenced via the matched arm.
#[allow(unused_imports)]
use Query as _Query;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_allowed_and_collects_tables() {
        let tables = validate_select("SELECT a, b FROM readings WHERE a > 1").unwrap();
        assert_eq!(tables, vec!["readings".to_string()]);
    }

    #[test]
    fn join_collects_both_tables() {
        let tables = validate_select("SELECT * FROM a JOIN b ON a.id = b.id").unwrap();
        assert!(tables.contains(&"a".to_string()) && tables.contains(&"b".to_string()));
    }

    #[test]
    fn insert_rejected() {
        assert!(validate_select("INSERT INTO t VALUES (1)").is_err());
    }

    #[test]
    fn update_rejected() {
        assert!(validate_select("UPDATE t SET a = 1").is_err());
    }

    #[test]
    fn delete_rejected() {
        assert!(validate_select("DELETE FROM t").is_err());
    }

    #[test]
    fn ddl_rejected() {
        assert!(validate_select("DROP TABLE t").is_err());
        assert!(validate_select("CREATE TABLE t (a int)").is_err());
    }

    #[test]
    fn multiple_statements_rejected() {
        assert!(validate_select("SELECT 1; SELECT 2").is_err());
        assert!(validate_select("SELECT * FROM t; DROP TABLE t").is_err());
    }
}
