//! `federation.delete` engine (entity-binding scope, O-2): a bounded, structured row DELETE from a
//! registered source's table. The caller supplies STRUCTURED key values — column names + a list of
//! key-aligned value rows — and NEVER SQL; the `DELETE ... WHERE k1=? AND k2=? ...` is GENERATED
//! here, parameterized at the driver level (no injection surface), and validated identifier by
//! identifier before it reaches the source.
//!
//! Row-capped (1 000 rows), mirroring `federation.write` exactly. Past the cap the answer is a typed
//! error string the host maps to `BadInput` — a synchronous tool handler never blocks on an
//! unbounded delete. This is NOT a tombstone hack: it is a real, bounded DELETE against the external
//! source, reached through the SAME `connect` path as `federation.query`/`federation.write`.
//!
//! The DSN is mediated by the host (handed in the call input, never logged/returned). The delete
//! goes through `Source::delete_rows`.

use crate::pool::cached_connect;
use serde_json::Value;

/// The hard cap on rows a single `federation.delete` accepts — the same bound as `federation.write`.
/// Past this a synchronous tool handler would block on an unbounded delete, which the scope forbids.
pub const ROW_CAP: usize = 1_000;

/// Run a bounded delete against the `kind` source at `dsn`. `key` names the identifying columns;
/// each entry in `rows` is a `key`-aligned array of values, so one row deletes every DB row matching
/// (`key` columns = the given values). All the DELETEs run in ONE transaction. Returns `{affected}`.
pub async fn run_delete(
    kind: &str,
    dsn: &str,
    table: &str,
    key: &[String],
    rows: &[Value],
) -> Result<u64, String> {
    if rows.len() > ROW_CAP {
        return Err(format!(
            "federation.delete is bounded to {ROW_CAP} rows (got {}); split the delete into \
             bounded batches",
            rows.len()
        ));
    }

    // Structural guards: the caller names a table + key columns; we never accept SQL. Validate the
    // identifiers so a generated statement can't break out of its quotes.
    validate_identifier(table).map_err(|e| format!("bad table name: {e}"))?;
    if key.is_empty() {
        return Err("key is empty — name at least one identifying column".into());
    }
    for c in key {
        validate_identifier(c).map_err(|e| format!("bad key column `{c}`: {e}"))?;
    }

    // Decompose the JSON rows into typed cell arrays. Each row must be an array, key-aligned to
    // `key` (the caller's contract).
    let mut typed_rows: Vec<Vec<Value>> = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let arr = row.as_array().ok_or_else(|| {
            format!("rows[{i}] is not an array (each row must be a key-aligned array)")
        })?;
        if arr.len() != key.len() {
            return Err(format!(
                "rows[{i}] has {} cells but {} key columns were named",
                arr.len(),
                key.len()
            ));
        }
        typed_rows.push(arr.clone());
    }

    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;
    let affected = source
        .delete_rows(table, key, &typed_rows)
        .await
        .map_err(|e| e.to_string())?;

    // Write-through invalidation (federation-result-cache scope): this source's rows just changed,
    // so every cached RESULT computed from it is now known-wrong. Dropping them here rather than in
    // the `main.rs` dispatch means any path that deletes invalidates — the guarantee belongs to the
    // delete, not to one caller of it. Coarse by source, exactly as `run_write` does.
    crate::results::evict_source(kind, dsn);
    Ok(affected)
}

/// Reject an identifier that is not `[a-zA-Z_][a-zA-Z0-9_]*` — the SAME injection guard
/// `federation.write` runs in front of the generated SQL. Even though the generator quotes
/// identifiers, a name with an embedded `"` could still break out. Defense in depth.
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
