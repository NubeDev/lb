//! Build the filtered derived table that stands in for one policy table:
//! `SELECT * FROM t WHERE (cols) IN (SELECT key cols FROM entity_key WHERE key_col = <key> AND
//! value_col IN (<ids>))`, or `WHERE col IN (<ids>)` for a table that carries the id directly.
//!
//! The shape is parsed from a template whose identifiers are policy-checked plain identifiers
//! (`RowPolicy::check`); the key value and every entity id are then written into the AST as string
//! literal VALUES. An id can contain quotes, spaces or SQL — it is data, never text.

use std::ops::ControlFlow;

use datafusion::sql::sqlparser::ast::{
    Expr, Query, Statement, Value, ValueWithSpan, VisitMut, VisitorMut,
};
use datafusion::sql::sqlparser::dialect::PostgreSqlDialect;
use datafusion::sql::sqlparser::parser::Parser;

use super::{RowScope, TableRule};
use crate::validate::ValidationError;

const KEY_MARK: &str = "__lb_row_key__";
const ID_MARK: &str = "__lb_row_id__";

pub(super) fn filtered(
    table: &str,
    rule: &TableRule,
    scope: &RowScope,
) -> Result<Box<Query>, ValidationError> {
    let template = if scope.ids.is_empty() {
        // Fail closed: no entities, no rows.
        format!("SELECT * FROM {table} WHERE false")
    } else {
        match rule {
            TableRule::Entity { column } => {
                format!("SELECT * FROM {table} WHERE {column} IN ('{ID_MARK}')")
            }
            TableRule::Keyed { columns } => {
                let ek = &scope.policy.entity_key;
                format!(
                    "SELECT * FROM {table} WHERE ({}) IN (SELECT {} FROM {} WHERE {} = '{KEY_MARK}' AND {} IN ('{ID_MARK}'))",
                    columns.join(", "),
                    ek.columns.join(", "),
                    ek.table,
                    ek.key_col,
                    ek.value_col,
                )
            }
        }
    };
    let mut statements = Parser::parse_sql(&PostgreSqlDialect {}, &template)
        .map_err(|e| ValidationError(format!("row policy template: {e}")))?;
    let Some(Statement::Query(mut query)) = statements.pop() else {
        return Err(ValidationError("row policy template is not a query".into()));
    };
    let mut fill = Fill { scope };
    let _ = query.visit(&mut fill);
    Ok(query)
}

fn literal(s: &str) -> Expr {
    Expr::Value(ValueWithSpan::from(Value::SingleQuotedString(
        s.to_string(),
    )))
}

struct Fill<'a> {
    scope: &'a RowScope,
}

impl VisitorMut for Fill<'_> {
    type Break = ();

    fn post_visit_value(&mut self, value: &mut Value) -> ControlFlow<()> {
        if matches!(value, Value::SingleQuotedString(s) if s == KEY_MARK) {
            *value = Value::SingleQuotedString(self.scope.policy.entity_key.key_value.clone());
        }
        ControlFlow::Continue(())
    }

    fn post_visit_expr(&mut self, expr: &mut Expr) -> ControlFlow<()> {
        if let Expr::InList { list, .. } = expr {
            let is_mark = matches!(list.as_slice(),
                [Expr::Value(v)] if matches!(&v.value, Value::SingleQuotedString(s) if s == ID_MARK));
            if is_mark {
                *list = self.scope.ids.iter().map(|id| literal(id)).collect();
            }
        }
        ControlFlow::Continue(())
    }
}
