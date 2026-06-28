//! Host-side SELECT-only pre-check (datasources scope: read-first v1, re-validated host-side too).
//! This is defense in depth IN FRONT of the sidecar's full `sqlparser` validation — core links no
//! SQL parser (the heavy dep lives only in the extension), so the host does a conservative keyword
//! gate: the statement must begin with `SELECT`/`WITH` and carry no second statement and no
//! data-modifying/DDL keyword as a leading token. A borderline-but-malicious query that slips this
//! gate is caught by the sidecar's real parser before it touches a pool — two independent gates.

use super::error::FederationError;

/// The leading keywords that mark a write or DDL — rejected outright (read-first v1).
const FORBIDDEN_LEADERS: &[&str] = &[
    "INSERT", "UPDATE", "DELETE", "MERGE", "CREATE", "DROP", "ALTER", "TRUNCATE", "COPY", "GRANT",
    "REVOKE", "CALL", "EXEC", "EXECUTE", "REPLACE", "UPSERT",
];

/// Reject `sql` unless it is a single read query. Conservative by design — the sidecar's parser is
/// authoritative; this stops the obvious writes/DDL and multi-statement injection at the host edge.
pub fn validate_select_host(sql: &str) -> Result<(), FederationError> {
    let trimmed = sql.trim().trim_end_matches(';').trim();
    if trimmed.is_empty() {
        return Err(FederationError::BadSql("empty".into()));
    }
    // No second statement: a `;` that still has non-whitespace after it is a multi-statement.
    if let Some(idx) = trimmed.find(';') {
        if !trimmed[idx + 1..].trim().is_empty() {
            return Err(FederationError::BadSql("multiple statements".into()));
        }
    }
    let upper = trimmed.to_ascii_uppercase();
    let leader = upper.split_whitespace().next().unwrap_or("");
    if FORBIDDEN_LEADERS.contains(&leader) {
        return Err(FederationError::BadSql(format!("{leader} not allowed")));
    }
    if leader != "SELECT" && leader != "WITH" {
        return Err(FederationError::BadSql("only SELECT/WITH allowed".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select() {
        assert!(validate_select_host("SELECT * FROM t").is_ok());
        assert!(validate_select_host("  select a from t ; ").is_ok());
        assert!(validate_select_host("WITH x AS (SELECT 1) SELECT * FROM x").is_ok());
    }

    #[test]
    fn rejects_writes_and_ddl() {
        for s in [
            "INSERT INTO t VALUES (1)",
            "UPDATE t SET a=1",
            "DELETE FROM t",
            "DROP TABLE t",
            "CREATE TABLE t (a int)",
        ] {
            assert!(validate_select_host(s).is_err(), "should reject: {s}");
        }
    }

    #[test]
    fn rejects_multi_statement() {
        assert!(validate_select_host("SELECT 1; DROP TABLE t").is_err());
    }
}
