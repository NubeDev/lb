//! The mutating AST walk: every relation → a policy-filtered derived table, every function → the
//! allow-list, every unknown FROM shape → refused. One pass; the derived tables it inserts are built
//! AFTER a factor's children are visited (`post_visit_table_factor`), so they are never re-walked.

use std::ops::ControlFlow;

use datafusion::sql::sqlparser::ast::{
    ArrayElemTypeDef, DataType, Expr, Ident, ObjectName, ObjectNamePart, Query, SetExpr,
    TableAlias, TableFactor, VisitMut, VisitorMut, With,
};

use super::{filter, functions, RowScope};
use crate::validate::ValidationError;

pub(super) struct Scoper<'a> {
    scope: &'a RowScope,
    /// CTE names visible at each open query level (innermost last). A table reference that names an
    /// in-scope CTE is left alone — the CTE body itself is rewritten where it is defined. A name is
    /// pushed only once Postgres would see it: for a plain `WITH`, AFTER its own body (so a body
    /// naming itself, or a later sibling, hits the REAL relation and is gated like one); for
    /// `WITH RECURSIVE`, before any body.
    ctes: Vec<Vec<String>>,
    /// Each open query's `with` clause, taken out while its CTE bodies are walked by hand
    /// (`pre_visit_query`) and restored afterwards (`post_visit_query`).
    withs: Vec<Option<With>>,
}

impl<'a> Scoper<'a> {
    pub(super) fn new(scope: &'a RowScope) -> Self {
        Self {
            scope,
            ctes: Vec::new(),
            withs: Vec::new(),
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

/// `SELECT ... INTO t` would CREATE a table from a read. Refused at the rewrite too (the cap wrapper
/// happens to reject it today; that must not be the only gate).
fn deny_select_into(body: &SetExpr) -> ControlFlow<ValidationError> {
    match body {
        SetExpr::Select(s) if s.into.is_some() => deny("SELECT INTO is not allowed"),
        SetExpr::SetOperation { left, right, .. } => {
            deny_select_into(left)?;
            deny_select_into(right)
        }
        _ => ControlFlow::Continue(()),
    }
}

/// Refuse a cast target that is not a built-in type.
fn deny_custom_type(t: &DataType) -> ControlFlow<ValidationError> {
    match t {
        DataType::Custom(name, _) => deny(format!("cast to {name} is not allowed")),
        DataType::Regclass => deny("cast to regclass is not allowed"),
        DataType::Array(
            ArrayElemTypeDef::AngleBracket(inner)
            | ArrayElemTypeDef::SquareBracket(inner, _)
            | ArrayElemTypeDef::Parenthesis(inner),
        ) => deny_custom_type(inner),
        _ => ControlFlow::Continue(()),
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
        if !query.locks.is_empty() {
            return deny("row locking is not allowed for a scoped read");
        }
        deny_select_into(&query.body)?;
        // Walk the CTE bodies BY HAND, in order, so each name becomes visible exactly when Postgres
        // makes it visible. The derived walk then sees no `with` (restored in `post_visit_query`).
        let mut with = query.with.take();
        self.ctes.push(Vec::new());
        if let Some(w) = with.as_mut() {
            let names: Vec<String> = w.cte_tables.iter().map(|c| folded(&c.alias.name)).collect();
            for name in &names {
                // A CTE named after a policy table — or after the ENTITY KEY table, which the
                // inserted predicates read by name — would shadow the real relation in Postgres.
                if self.scope.policy.tables.contains_key(name)
                    || *name == self.scope.policy.entity_key.table
                {
                    return deny(format!("a CTE may not shadow the table {name}"));
                }
            }
            if w.recursive {
                self.ctes
                    .last_mut()
                    .expect("level")
                    .extend(names.iter().cloned());
                for cte in w.cte_tables.iter_mut() {
                    cte.query.visit(self)?;
                }
            } else {
                for (i, cte) in w.cte_tables.iter_mut().enumerate() {
                    cte.query.visit(self)?;
                    self.ctes.last_mut().expect("level").push(names[i].clone());
                }
            }
        }
        self.withs.push(with);
        ControlFlow::Continue(())
    }

    fn post_visit_query(&mut self, query: &mut Query) -> ControlFlow<Self::Break> {
        query.with = self.withs.pop().flatten();
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
        // A cast to a catalog-object type (`'hosts'::regclass`) or to ANY custom type (every table is
        // also a composite type: `(NULL::hosts).col`) answers by whether a name EXISTS — a catalog
        // probe the relation gate would otherwise refuse. Only built-in types are allowed.
        match expr {
            Expr::Cast { data_type, .. } => deny_custom_type(data_type)?,
            Expr::TypedString(ts) => deny_custom_type(&ts.data_type)?,
            _ => {}
        }
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
