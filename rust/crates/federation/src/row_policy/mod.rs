//! Entity-scoped row policy (entity-scoped-data scope): narrow a restricted principal's SELECT to the
//! rows of the entities they may read, BEFORE it touches a pool or the result cache.
//!
//! The host decides WHO is restricted and WHICH entity ids they reach (server state only — menus and
//! grants, never URL, variables or tool args) and sends `row_scope: {policy, ids}` with the call. An
//! unrestricted call carries no `row_scope` and runs byte-for-byte as before.
//!
//! Deny by default, in three layers:
//! 1. every relation must be a POLICY table (or an in-scope CTE) and is replaced by a filtered
//!    derived table — `(SELECT * FROM t WHERE <entity predicate>) AS t` — so every column reference
//!    keeps resolving while only in-scope rows exist;
//! 2. every function must be on the allow-list ([`functions`]) — `query_to_xml`, `dblink`,
//!    `set_config`, `pg_*` and friends can run SQL or read state outside the rewrite;
//! 3. any FROM shape the rewrite does not understand (table functions, `TABLE x`, pivots, …) is
//!    refused.
//! Entity ids reach the AST only as string literal VALUES ([`filter`]); policy identifiers are
//! checked to be plain lowercase identifiers when the policy is read. Nothing is string-spliced.

mod filter;
mod functions;
mod relations;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;

use datafusion::sql::sqlparser::ast::{Statement, VisitMut};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use datafusion::sql::sqlparser::parser::Parser;
use serde::Deserialize;

use crate::validate::ValidationError;

/// What the host attaches to a restricted call.
#[derive(Debug, Clone, Deserialize)]
pub struct RowScope {
    pub policy: RowPolicy,
    /// The entity ids the principal may read. Empty = nothing (fail closed, never "all").
    pub ids: Vec<String>,
}

/// A datasource's row policy, authored by an admin (host `federation.row_policy_set`).
#[derive(Debug, Clone, Deserialize)]
pub struct RowPolicy {
    /// Readable relations → how each one maps to an entity. Keys are lowercase table names.
    pub tables: BTreeMap<String, TableRule>,
    /// How a keyed row resolves to its entity id (e.g. a point's `siteRef` tag row).
    pub entity_key: EntityKey,
    /// Extra function names the policy allows on top of the built-in allow-list.
    #[serde(default)]
    pub extra_functions: Vec<String>,
}

/// How one readable table is narrowed.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum TableRule {
    /// Rows keyed by `columns` (e.g. `host_uuid, point_uuid`), resolved to an entity through
    /// [`EntityKey`].
    Keyed { columns: Vec<String> },
    /// Rows that carry the entity id directly in `column`.
    Entity { column: String },
}

/// The lookup that maps keyed rows to entity ids: rows of `table` whose `key_col = key_value` carry
/// the entity id in `value_col`, keyed by `columns` (matched positionally to a [`TableRule::Keyed`]).
#[derive(Debug, Clone, Deserialize)]
pub struct EntityKey {
    pub table: String,
    pub columns: Vec<String>,
    pub key_col: String,
    pub key_value: String,
    pub value_col: String,
}

/// Parse `row_scope` from the call input. Absent → `None` (unrestricted call). Present but malformed
/// or unsafe → an error: a restricted call never degrades to an unrestricted one.
pub fn from_input(input: &serde_json::Value) -> Result<Option<RowScope>, ValidationError> {
    let Some(raw) = input.get("row_scope") else {
        return Ok(None);
    };
    let scope: RowScope = serde_json::from_value(raw.clone())
        .map_err(|e| ValidationError(format!("bad row_scope: {e}")))?;
    scope.policy.check()?;
    Ok(Some(scope))
}

impl RowPolicy {
    /// Every identifier the policy injects into SQL must be a plain lowercase identifier.
    fn check(&self) -> Result<(), ValidationError> {
        let ek = &self.entity_key;
        let mut idents: Vec<&str> = vec![&ek.table, &ek.key_col, &ek.value_col];
        idents.extend(ek.columns.iter().map(String::as_str));
        for (name, rule) in &self.tables {
            idents.push(name);
            match rule {
                TableRule::Keyed { columns } => {
                    if columns.len() != ek.columns.len() || columns.is_empty() {
                        return Err(ValidationError(format!(
                            "row policy: table {name} keys do not match the entity key"
                        )));
                    }
                    idents.extend(columns.iter().map(String::as_str));
                }
                TableRule::Entity { column } => idents.push(column),
            }
        }
        if let Some(bad) = idents.into_iter().find(|s| !plain_ident(s)) {
            return Err(ValidationError(format!(
                "row policy: bad identifier {bad:?}"
            )));
        }
        Ok(())
    }
}

fn plain_ident(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_lowercase())
        && chars.all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}

/// Rewrite an already-expanded SELECT under `scope`. Returns the SQL the source must run instead.
pub fn rewrite(sql: &str, scope: &RowScope) -> Result<String, ValidationError> {
    let dialect = PostgreSqlDialect {};
    let mut statements = Parser::parse_sql(&dialect, sql)
        .map_err(|e| ValidationError(format!("parse error: {e}")))?;
    if statements.len() != 1 {
        return Err(ValidationError("exactly one statement allowed".into()));
    }
    let Statement::Query(query) = &mut statements[0] else {
        return Err(ValidationError("only a read query is allowed".into()));
    };
    let mut visitor = relations::Scoper::new(scope);
    if let std::ops::ControlFlow::Break(e) = query.visit(&mut visitor) {
        return Err(e);
    }
    Ok(statements[0].to_string())
}
