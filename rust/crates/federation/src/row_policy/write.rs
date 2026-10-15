//! Entity-scoped WRITES (`entity-scoped-writes-scope.md`): turn the host's `write_scope` into the
//! guard a structured write or delete runs under, and check the rows as written before anything runs.
//!
//! The host sends `write_scope {policy, ids}` only for a restricted caller writing a table its policy
//! marks `writable`; an unrestricted call carries none and writes exactly as before. The sidecar
//! re-checks everything (the second, independent gate): the policy parses and passes the identifier
//! check, the table is a writable `entity` rule, the set is non-empty, and every row names the entity
//! column with one of the ids. Anything else is refused, never written unscoped.

use serde_json::{json, Value};

use super::{from_input, TableRule};
use crate::source::WriteGuard;

const DENIED: &str =
    "denied: this table is not writable for a caller limited to their own entities";

/// The guard for writing `table` under `scope` (the call's `write_scope`). `None` scope ⇒ unrestricted.
pub fn guard(scope: Option<&Value>, table: &str) -> Result<Option<WriteGuard>, String> {
    let Some(raw) = scope else {
        return Ok(None);
    };
    let scope = from_input(&json!({ "row_scope": raw }))
        .map_err(|e| format!("bad write_scope: {}", e.0))?
        .ok_or_else(|| DENIED.to_string())?;
    match scope.policy.tables.get(&table.to_lowercase()) {
        Some(TableRule::Entity {
            column,
            writable: true,
        }) if !scope.ids.is_empty() => Ok(Some(WriteGuard {
            column: column.clone(),
            ids: scope.ids,
        })),
        _ => Err(DENIED.into()),
    }
}

/// `federation.write` under the call's `write_scope` (read off `call`, the tool input): the rows are
/// checked, then written with the guard.
pub async fn write(
    (kind, dsn, table): (&str, &str, &str),
    columns: &[String],
    rows: &[Value],
    key: Option<&[String]>,
    call: &Value,
) -> Result<u64, String> {
    let guard = guard(call.get("write_scope"), table)?;
    if let Some(g) = &guard {
        check_rows(g, columns, rows)?;
    }
    crate::write::run_write(kind, dsn, table, columns, rows, key, guard.as_ref()).await
}

/// `federation.delete` under the call's `write_scope` (read off `call`): only the caller's entities'
/// rows are deleted.
pub async fn delete(
    (kind, dsn, table): (&str, &str, &str),
    key: &[String],
    rows: &[Value],
    call: &Value,
) -> Result<u64, String> {
    let guard = guard(call.get("write_scope"), table)?;
    crate::delete::run_delete(kind, dsn, table, key, rows, guard.as_ref()).await
}

/// Every row as written must carry the entity column, holding one of the guard's ids.
pub fn check_rows(guard: &WriteGuard, columns: &[String], rows: &[Value]) -> Result<(), String> {
    let at = columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case(&guard.column))
        .ok_or_else(|| format!("denied: a scoped write must set `{}`", guard.column))?;
    for (i, row) in rows.iter().enumerate() {
        match row.get(at).and_then(Value::as_str) {
            Some(id) if guard.ids.iter().any(|g| g == id) => {}
            _ => {
                return Err(format!(
                    "denied: rows[{i}] is not one of the caller's entities"
                ))
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope(writable: bool, ids: &[&str]) -> Value {
        json!({
            "policy": {
                "tables": { "setting": { "kind": "entity", "column": "site_id", "writable": writable } },
                "entity_key": { "table": "tags", "columns": ["id"], "key_col": "k", "key_value": "site", "value_col": "v" }
            },
            "ids": ids,
        })
    }

    #[test]
    fn no_scope_is_unrestricted() {
        assert_eq!(guard(None, "setting").unwrap(), None);
    }

    #[test]
    fn a_writable_entity_table_gives_a_guard() {
        let g = guard(Some(&scope(true, &["a"])), "Setting")
            .unwrap()
            .unwrap();
        assert_eq!(
            (g.column.as_str(), g.ids.as_slice()),
            ("site_id", &["a".to_string()][..])
        );
    }

    #[test]
    fn everything_else_is_refused() {
        assert!(
            guard(Some(&scope(false, &["a"])), "setting").is_err(),
            "not writable"
        );
        assert!(
            guard(Some(&scope(true, &[])), "setting").is_err(),
            "empty set"
        );
        assert!(
            guard(Some(&scope(true, &["a"])), "other").is_err(),
            "not a policy table"
        );
        assert!(
            guard(Some(&json!({ "ids": ["a"] })), "setting").is_err(),
            "malformed"
        );
    }

    #[test]
    fn rows_must_name_one_of_the_ids() {
        let g = WriteGuard {
            column: "site_id".into(),
            ids: vec!["a".into()],
        };
        let cols = vec!["id".to_string(), "site_id".to_string()];
        assert!(check_rows(&g, &cols, &[json!(["x", "a"])]).is_ok());
        assert!(
            check_rows(&g, &cols, &[json!(["x", "a"]), json!(["y", "b"])]).is_err(),
            "one stranger"
        );
        assert!(
            check_rows(&g, &cols, &[json!(["x", null])]).is_err(),
            "no entity"
        );
        assert!(
            check_rows(&g, &["id".to_string()], &[json!(["x"])]).is_err(),
            "column missing"
        );
    }
}
