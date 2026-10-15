//! The generated SQL of a structured `federation.write` / `.delete`, shared by the sqlite and
//! postgres sources (they differed only in placeholder style), and the entity-scope guard on it
//! (`entity-scoped-writes-scope.md`).
//!
//! Under a [`WriteGuard`] the statement itself enforces the scope, so the check and the write are one
//! atomic step: an upsert only UPDATES an existing row whose entity column is in the caller's set (the
//! ids bind after the row's own cells), and a delete only removes such rows. The source checks each
//! guarded upsert changed exactly one row and rolls the whole batch back otherwise; that zero is the
//! "an out-of-scope row already holds this key" case. Identifiers are quoted here and validated
//! caller-side; every value is a bound parameter.

use super::dialect::quote_ident;

/// A write restricted to the caller's entities: the table's `column` must hold one of `ids`, on the
/// row as written (checked before the statement runs) and on any row it updates or deletes.
#[derive(Debug, Clone, PartialEq)]
pub struct WriteGuard {
    pub column: String,
    pub ids: Vec<String>,
}

/// How a source numbers its bound parameters.
#[derive(Debug, Clone, Copy)]
pub enum Placeholder {
    /// sqlite: `?1`, `?2`, …
    Numbered,
    /// postgres: `$1`, `$2`, …
    Dollar,
}

fn ph(style: Placeholder, n: usize) -> String {
    match style {
        Placeholder::Numbered => format!("?{n}"),
        Placeholder::Dollar => format!("${n}"),
    }
}

/// `"table"."column" IN (<ids>)`, the ids numbered from `first`.
fn in_scope(table: &str, guard: &WriteGuard, first: usize, style: Placeholder) -> String {
    let ids: Vec<String> = (0..guard.ids.len()).map(|i| ph(style, first + i)).collect();
    format!(
        "{}.{} IN ({})",
        quote_ident(table),
        quote_ident(&guard.column),
        ids.join(", ")
    )
}

/// `INSERT … VALUES (…)` and, with a `key`, `ON CONFLICT(key) DO UPDATE SET …` (`DO NOTHING` when
/// every column is a key). Guarded, the update gains `WHERE <entity> IN (<ids>)`, and an all-key write
/// sets its first key column to itself instead of `DO NOTHING`, so an in-scope conflict still counts
/// as the one changed row the source checks for.
pub fn upsert(
    table: &str,
    columns: &[String],
    key: &[String],
    guard: Option<&WriteGuard>,
    style: Placeholder,
) -> String {
    let cols: Vec<String> = columns.iter().map(|c| quote_ident(c)).collect();
    let values: Vec<String> = (1..=columns.len()).map(|i| ph(style, i)).collect();
    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        quote_ident(table),
        cols.join(", "),
        values.join(", ")
    );
    if key.is_empty() {
        return sql;
    }
    let conflict: Vec<String> = key.iter().map(|c| quote_ident(c)).collect();
    let mut updates: Vec<String> = columns
        .iter()
        .filter(|c| !key.contains(c))
        .map(|c| format!("{0}=excluded.{0}", quote_ident(c)))
        .collect();
    if updates.is_empty() {
        if guard.is_none() {
            return format!("{sql} ON CONFLICT({}) DO NOTHING", conflict.join(", "));
        }
        updates.push(format!("{0}=excluded.{0}", conflict[0]));
    }
    sql.push_str(&format!(
        " ON CONFLICT({}) DO UPDATE SET {}",
        conflict.join(", "),
        updates.join(", ")
    ));
    if let Some(guard) = guard {
        sql.push_str(&format!(
            " WHERE {}",
            in_scope(table, guard, columns.len() + 1, style)
        ));
    }
    sql
}

/// `DELETE FROM … WHERE k1=… AND …`, plus `AND <entity> IN (<ids>)` under a guard: a delete never
/// touches a row outside the caller's set (a key held by such a row simply deletes nothing).
pub fn delete(
    table: &str,
    key: &[String],
    guard: Option<&WriteGuard>,
    style: Placeholder,
) -> String {
    let mut terms: Vec<String> = key
        .iter()
        .enumerate()
        .map(|(i, c)| format!("{}={}", quote_ident(c), ph(style, i + 1)))
        .collect();
    if let Some(guard) = guard {
        terms.push(in_scope(table, guard, key.len() + 1, style));
    }
    format!(
        "DELETE FROM {} WHERE {}",
        quote_ident(table),
        terms.join(" AND ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }
    fn guard() -> WriteGuard {
        WriteGuard {
            column: "site_id".into(),
            ids: s(&["a", "b"]),
        }
    }

    #[test]
    fn an_unguarded_upsert_is_unchanged() {
        let cols = s(&["id", "site_id", "value"]);
        assert_eq!(
            upsert("setting", &cols, &s(&["id"]), None, Placeholder::Numbered),
            r#"INSERT INTO "setting" ("id", "site_id", "value") VALUES (?1, ?2, ?3) ON CONFLICT("id") DO UPDATE SET "site_id"=excluded."site_id", "value"=excluded."value""#
        );
        assert_eq!(
            upsert(
                "setting",
                &s(&["id"]),
                &s(&["id"]),
                None,
                Placeholder::Dollar
            ),
            r#"INSERT INTO "setting" ("id") VALUES ($1) ON CONFLICT("id") DO NOTHING"#
        );
        assert_eq!(
            upsert("t", &s(&["id"]), &[], None, Placeholder::Dollar),
            r#"INSERT INTO "t" ("id") VALUES ($1)"#
        );
    }

    #[test]
    fn a_guarded_upsert_updates_only_an_in_scope_row() {
        let cols = s(&["id", "site_id", "value"]);
        assert_eq!(
            upsert(
                "setting",
                &cols,
                &s(&["id"]),
                Some(&guard()),
                Placeholder::Dollar
            ),
            r#"INSERT INTO "setting" ("id", "site_id", "value") VALUES ($1, $2, $3) ON CONFLICT("id") DO UPDATE SET "site_id"=excluded."site_id", "value"=excluded."value" WHERE "setting"."site_id" IN ($4, $5)"#
        );
        // All key columns: a no-op self-update rather than DO NOTHING, so an in-scope conflict changes one row.
        assert_eq!(
            upsert(
                "t",
                &s(&["site_id", "name"]),
                &s(&["site_id", "name"]),
                Some(&guard()),
                Placeholder::Numbered
            ),
            r#"INSERT INTO "t" ("site_id", "name") VALUES (?1, ?2) ON CONFLICT("site_id", "name") DO UPDATE SET "site_id"=excluded."site_id" WHERE "t"."site_id" IN (?3, ?4)"#
        );
    }

    #[test]
    fn a_guarded_delete_only_reaches_in_scope_rows() {
        assert_eq!(
            delete("t", &s(&["id"]), None, Placeholder::Numbered),
            r#"DELETE FROM "t" WHERE "id"=?1"#
        );
        assert_eq!(
            delete("t", &s(&["id"]), Some(&guard()), Placeholder::Dollar),
            r#"DELETE FROM "t" WHERE "id"=$1 AND "t"."site_id" IN ($2, $3)"#
        );
    }
}
