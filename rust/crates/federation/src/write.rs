//! `federation.write` engine (schema-designer scope): a bounded INSERT/UPSERT of structured rows
//! into a registered source's table. The rows come from the caller as JSON; the SQL is GENERATED
//! here (the caller never supplies SQL), parameterized at the driver level (no injection surface),
//! and a write-validator re-parses the generated statement to assert it is exactly one INSERT with
//! no DDL/DML/multi-statement shape — defense in depth alongside the structural identifier check.
//!
//! Row-capped (start: 1 000 rows). Past the cap the answer is "use federation.export" — returned
//! as a typed error string so the host can map it to `BadInput` (the scope's posture: a
//! synchronous tool handler never blocks on an unbounded write).
//!
//! The DSN is mediated by the host (handed in the call input, never logged/returned). Reuses the
//! SAME `connect` path as `federation.query`; the write goes through `Source::write_rows`.

use crate::pool::cached_connect;
use serde_json::Value;
/// The hard cap on rows a single `federation.write` accepts. Past this the caller must use
/// `federation.export` (a durable job) — a synchronous tool handler never blocks on an unbounded
/// write (scope Intent: "Bulk is a job; per-message is a verb", §6.1).
pub const ROW_CAP: usize = 1_000;

/// Run a bounded write against the `kind` source at `dsn`. `rows` is a column-aligned array of
/// arrays (each inner array is one row's values, in `columns` order). When `key` names conflict
/// columns, the write is an UPSERT — idempotent under redelivery (a flow firing twice writes the
/// same row once). Returns `{affected}`.
pub async fn run_write(
    kind: &str,
    dsn: &str,
    table: &str,
    columns: &[String],
    rows: &[Value],
    key: Option<&[String]>,
) -> Result<u64, String> {
    if rows.len() > ROW_CAP {
        return Err(format!(
            "federation.write is bounded to {ROW_CAP} rows (got {}); use federation.export for a \
             durable bulk write",
            rows.len()
        ));
    }

    // Structural guards: the caller names a table + columns; we never accept SQL. Validate the
    // identifiers so a generated statement can't break out of its quotes.
    validate_identifier(table).map_err(|e| format!("bad table name: {e}"))?;
    for c in columns {
        validate_identifier(c).map_err(|e| format!("bad column name `{c}`: {e}"))?;
    }
    if let Some(key) = key {
        for c in key {
            validate_identifier(c).map_err(|e| format!("bad key column `{c}`: {e}"))?;
            if !columns.contains(c) {
                return Err(format!("key column `{c}` not in the write columns"));
            }
        }
    }

    // Decompose the JSON rows into typed cell arrays. Each row must be an array, column-aligned
    // to `columns` (the caller's contract).
    let mut typed_rows: Vec<Vec<Value>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let arr = row.as_array().ok_or_else(|| {
            format!("rows[{i}] is not an array (each row must be a column-aligned array)")
        })?;
        if arr.len() != columns.len() {
            return Err(format!(
                "rows[{i}] has {} cells but {} columns were named",
                arr.len(),
                columns.len()
            ));
        }
        typed_rows.push(arr.clone());
    }

    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
    let affected = source
        .write_rows(table, columns, &typed_rows, key)
        .await
        .map_err(|e| e.to_string())?;
    Ok(affected)
}

/// Reject an identifier that is not `[a-zA-Z_][a-zA-Z0-9_]*`. This is the injection guard in front
/// of the generated SQL — even though the generator quotes identifiers, a name with an embedded
/// `"` could still break out. Defense in depth: the design record also validates at save.
fn validate_identifier(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("empty".into());
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!("`{name}` must start with a letter or underscore"));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "`{name}` may contain only letters, digits, or underscore"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_validation_rejects_injection() {
        assert!(validate_identifier("users").is_ok());
        assert!(validate_identifier("user_id").is_ok());
        assert!(validate_identifier("_private").is_ok());
        // Injection vectors:
        assert!(validate_identifier("").is_err());
        assert!(validate_identifier("user\"; DROP").is_err());
        assert!(validate_identifier("1users").is_err());
        assert!(validate_identifier("user-name").is_err());
        assert!(validate_identifier("user; DROP TABLE t").is_err());
    }
}
