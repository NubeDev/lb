//! Postgres `numeric` on the direct read path — asking the server for `float8` so a value cannot
//! depend on which row it arrived in.
//!
//! The connector builds ONE Arrow `Decimal128` column per result and fixes its scale from the FIRST
//! non-null row it decodes, then rescales every later row down to that scale — a lossy round, not a
//! reinterpretation. A result whose first row happens to be a whole number therefore truncates the
//! entire column: `20, 17.685, 15.334` decodes as `20.0, 18.0, 15.0`, while the identical values
//! reordered decode exactly. A single-row result is always right, so the loss is invisible until a
//! chart or a threshold reads a window whose newest sample has no decimals.
//!
//! Only an UNCONSTRAINED `numeric` is exposed. A declared `numeric(10,3)` carries its scale in the
//! catalog and the connector pins the column from there — but `max()`, `min()`, `avg()` and any
//! arithmetic over a numeric column all return unconstrained `numeric`, which is most of what a
//! panel or a rule actually selects.
//!
//! The only place a scale can be chosen safely is the server, which knows every row before it sends
//! any: so we ask it for `double precision` instead. That is not a lossy trade here — the JSON cell
//! this path ultimately emits is an f64 whichever type arrives (a `Decimal128` cell is divided down
//! to f64 on the way out), so casting server-side keeps exactly what the caller could ever have
//! received and drops only a rounding that depended on row order. Widening the Arrow scale instead
//! would need the connector to look at the whole batch before choosing one, which it does not do,
//! and pinning a fixed generous scale would silently misplace the decimal point on values too large
//! to rescale that far.
//!
//! The cast is applied by wrapping the query, so the projection is rebuilt POSITIONALLY
//! (`lb_numeric(c0, c1, …)`) and the original column names are restored as aliases: a result with
//! duplicate or unnamed columns (`?column?`, two `value`s from a join) still wraps correctly, which
//! a name-based projection could not do.

use datafusion_table_providers::sql::db_connection_pool::postgrespool::PostgresConnectionPool;
use tokio_postgres::types::Type;

/// The SQL to run in place of `sql` so no `numeric` column reaches the Arrow decoder, or `None` when
/// the result has no such column (the overwhelmingly common case) and `sql` should run as written.
///
/// The result's column types come from PREPARING the statement — the server reports them without
/// executing anything, and the prepare runs on a pooled connection, so this costs one parse
/// round-trip and never a second execution of the caller's query.
///
/// Any failure here yields `None`: the original SQL then runs and reports its own error, so a broken
/// query fails exactly as it did before rather than with a message about a rewrite the caller never
/// asked for.
pub(super) async fn widen_numeric_columns(
    pool: &PostgresConnectionPool,
    sql: &str,
) -> Option<String> {
    let conn = pool.connect_direct().await.ok()?;
    let stmt = conn.conn.prepare(sql).await.ok()?;
    let columns: Vec<(String, Option<&'static str>)> = stmt
        .columns()
        .iter()
        .map(|c| (c.name().to_string(), cast_target(c.type_())))
        .collect();
    if columns.iter().all(|(_, cast)| cast.is_none()) {
        return None;
    }
    Some(wrap_with_casts(sql, &columns))
}

/// The type to cast a result column to, or `None` to pass it through untouched. A `numeric[]` has
/// the same first-row scale applied element-wise, so it takes the same treatment.
fn cast_target(ty: &Type) -> Option<&'static str> {
    match *ty {
        Type::NUMERIC => Some("float8"),
        Type::NUMERIC_ARRAY => Some("float8[]"),
        _ => None,
    }
}

/// Wrap `sql` in a projection that casts the flagged columns and restores every original name.
/// The inner query is aliased positionally, so nothing here depends on its columns being uniquely
/// (or even sensibly) named. Output names are quoted with `"` doubled, so a column called `a"b`
/// survives.
fn wrap_with_casts(sql: &str, columns: &[(String, Option<&'static str>)]) -> String {
    let mut projection = Vec::with_capacity(columns.len());
    let mut positional = Vec::with_capacity(columns.len());
    for (i, (name, cast)) in columns.iter().enumerate() {
        let src = format!("c{i}");
        let out = name.replace('"', "\"\"");
        projection.push(match cast {
            Some(ty) => format!("{src}::{ty} AS \"{out}\""),
            None => format!("{src} AS \"{out}\""),
        });
        positional.push(src);
    }
    format!(
        "SELECT {}\nFROM (\n{}\n) AS lb_numeric({})",
        projection.join(", "),
        sql.trim(),
        positional.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_flagged_columns_are_cast_and_names_are_restored() {
        let sql = wrap_with_casts(
            "SELECT ts, v FROM readings",
            &[("ts".into(), None), ("v".into(), Some("float8"))],
        );
        assert!(sql.contains("c0 AS \"ts\""), "{sql}");
        assert!(sql.contains("c1::float8 AS \"v\""), "{sql}");
        assert!(sql.contains(") AS lb_numeric(c0, c1)"), "{sql}");
    }

    /// Duplicate and unnamed output columns are legal in Postgres; the positional alias list is what
    /// makes them wrap. A name-based projection would be ambiguous here and fail the whole query.
    #[test]
    fn duplicate_and_odd_column_names_still_wrap() {
        let sql = wrap_with_casts(
            "SELECT a.v, b.v, 1 FROM a JOIN b ON true",
            &[
                ("v".into(), Some("float8")),
                ("v".into(), Some("float8")),
                ("?column?".into(), None),
            ],
        );
        assert!(
            sql.contains("c0::float8 AS \"v\", c1::float8 AS \"v\""),
            "{sql}"
        );
        assert!(sql.contains("c2 AS \"?column?\""), "{sql}");
    }

    #[test]
    fn a_quote_in_a_column_name_is_escaped() {
        let sql = wrap_with_casts("SELECT 1", &[("a\"b".into(), Some("float8"))]);
        assert!(sql.contains("c0::float8 AS \"a\"\"b\""), "{sql}");
    }

    #[test]
    fn numeric_arrays_take_the_same_cast() {
        assert_eq!(cast_target(&Type::NUMERIC), Some("float8"));
        assert_eq!(cast_target(&Type::NUMERIC_ARRAY), Some("float8[]"));
        assert_eq!(cast_target(&Type::FLOAT8), None);
        assert_eq!(cast_target(&Type::TEXT), None);
    }
}
