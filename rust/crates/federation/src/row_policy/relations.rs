//! The mutating AST walk: every relation → a policy-filtered derived table, every function → the
//! allow-list, every unknown FROM shape → refused. One pass; the derived tables it inserts are built
//! AFTER a factor's children are visited (`post_visit_table_factor`), so they are never re-walked.

use std::ops::ControlFlow;

use datafusion::sql::sqlparser::ast::{
    Expr, Ident, ObjectName, ObjectNamePart, Query, SetExpr, TableAlias, TableFactor, VisitorMut,
};

use super::{filter, functions, RowScope};
use crate::validate::ValidationError;

pub(super) struct Scoper<'a> {
    scope: &'a RowScope,
    /// CTE names visible at each open query level (innermost last). A table reference that names an
    /// in-scope CTE is left alone — the CTE body itself is rewritten where it is defined.
    ctes: Vec<Vec<String>>,
}

impl<'a> Scoper<'a> {
    pub(super) fn new(scope: &'a RowScope) -> Self {
        Self {
            scope,
            ctes: Vec::new(),
        }
    }

    fn is_cte(&self, name: &str) -> bool {
        self.ctes
            .iter()
            .any(|level| level.iter().any(|c| c == name))
    }
}

fn deny<T>(msg: impl Into<String>) -> ControlFlow<ValidationError, T> {
    ControlFlow::Break(ValidationError(msg.into()))
}

/// Postgres name folding: unquoted identifiers are lowercase, quoted ones are exact.
fn folded(ident: &Ident) -> String {
    match ident.quote_style {
        None => ident.value.to_lowercase(),
        Some(_) => ident.value.clone(),
    }
}

/// The relation a name refers to: `t` or `public.t` → `Some("t")`; anything else → `None`.
fn relation_name(name: &ObjectName) -> Option<(String, Ident)> {
    let idents: Vec<&Ident> = name
        .0
        .iter()
        .map(|p| match p {
            ObjectNamePart::Identifier(i) => Some(i),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    match idents.as_slice() {
        [t] => Some((folded(t), (*t).clone())),
        [schema, t] if folded(schema) == "public" => Some((folded(t), (*t).clone())),
        _ => None,
    }
}

/// `TABLE x` and other set-expression shapes that read a relation without a FROM factor.
fn check_set_expr(body: &SetExpr) -> ControlFlow<ValidationError> {
    match body {
        SetExpr::Select(_) | SetExpr::Query(_) | SetExpr::Values(_) => ControlFlow::Continue(()),
        SetExpr::SetOperation { left, right, .. } => {
            check_set_expr(left)?;
            check_set_expr(right)
        }
        _ => deny("this query shape is not allowed for a scoped read"),
    }
}

impl VisitorMut for Scoper<'_> {
    type Break = ValidationError;

    fn pre_visit_query(&mut self, query: &mut Query) -> ControlFlow<Self::Break> {
        check_set_expr(&query.body)?;
        let mut level = Vec::new();
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                let name = folded(&cte.alias.name);
                if self.scope.policy.tables.contains_key(&name) {
                    return deny(format!("a CTE may not shadow the table {name}"));
                }
                level.push(name);
            }
        }
        self.ctes.push(level);
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, _query: &mut Query) -> ControlFlow<Self::Break> {
        self.ctes.pop();
        ControlFlow::Continue(())
    }

    fn post_visit_table_factor(&mut self, factor: &mut TableFactor) -> ControlFlow<Self::Break> {
        match factor {
            TableFactor::Derived { .. } | TableFactor::NestedJoin { .. } => {
                ControlFlow::Continue(())
            }
            TableFactor::Table {
                name,
                args: Some(_),
                ..
            } => {
                // A set-returning function in FROM (`generate_series(...)`): its arguments were
                // already checked as expressions; the function itself must be allow-listed.
                match relation_name(name) {
                    Some((n, _)) if functions::set_returning(&n) => ControlFlow::Continue(()),
                    _ => deny(format!("function {name} is not allowed in FROM")),
                }
            }
            TableFactor::Table {
                name,
                alias,
                args: None,
                with_hints,
                version,
                partitions,
                json_path,
                sample,
                index_hints,
                with_ordinality,
            } => {
                if !with_hints.is_empty()
                    || version.is_some()
                    || !partitions.is_empty()
                    || json_path.is_some()
                    || sample.is_some()
                    || !index_hints.is_empty()
                    || *with_ordinality
                {
                    return deny(format!("table modifiers are not allowed on {name}"));
                }
                let Some((table, written)) = relation_name(name) else {
                    return deny(format!("relation {name} is not readable"));
                };
                if name.0.len() == 1 && self.is_cte(&table) {
                    return ControlFlow::Continue(());
                }
                let Some(rule) = self.scope.policy.tables.get(&table) else {
                    return deny(format!("relation {name} is not readable"));
                };
                let subquery = match filter::filtered(&table, rule, self.scope) {
                    Ok(q) => q,
                    Err(e) => return ControlFlow::Break(e),
                };
                let alias = alias.take().unwrap_or(TableAlias {
                    explicit: true,
                    name: written,
                    columns: vec![],
                });
                *factor = TableFactor::Derived {
                    lateral: false,
                    subquery,
                    alias: Some(alias),
                    sample: None,
                };
                ControlFlow::Continue(())
            }
            _ => deny("this FROM item is not allowed for a scoped read"),
        }
    }

    fn post_visit_expr(&mut self, expr: &mut Expr) -> ControlFlow<Self::Break> {
        if let Expr::Function(f) = expr {
            let allowed = match relation_name(&f.name) {
                // A schema-qualified call (`public.f`) folds to its bare name; any other qualifier
                // (`pg_catalog.f`, `dblink.f`) is refused by `relation_name`.
                Some((n, _)) => functions::allowed(&n, &self.scope.policy.extra_functions),
                None => false,
            };
            if !allowed {
                return deny(format!("function {} is not allowed", f.name));
            }
        }
        ControlFlow::Continue(())
    }
}
