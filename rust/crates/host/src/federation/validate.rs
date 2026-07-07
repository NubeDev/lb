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

/// Strip leading SQL comments (`-- line` and `/* block */`) and whitespace — a legitimate SELECT
/// prefixed by a comment (agents and humans both write `-- what this query does` headers) must not
/// be rejected by the leader-keyword gate. The sidecar's real parser accepts them; this gate must
/// not be stricter than the authoritative one on VALID input.
fn strip_leading_comments(mut s: &str) -> &str {
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix("--") {
            s = rest.split_once('\n').map(|(_, tail)| tail).unwrap_or("");
        } else if let Some(rest) = s.strip_prefix("/*") {
            match rest.split_once("*/") {
                Some((_, tail)) => s = tail,
                None => return "", // unterminated block comment — nothing runnable follows
            }
        } else {
            return s;
        }
    }
}

/// Reject `sql` unless it is a single read query. Conservative by design — the sidecar's parser is
/// authoritative; this stops the obvious writes/DDL and multi-statement injection at the host edge.
pub fn validate_select_host(sql: &str) -> Result<(), FederationError> {
    let trimmed = strip_leading_comments(sql)
        .trim()
        .trim_end_matches(';')
        .trim();
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
    // `information_schema.tables` / `information_schema.columns` are ALLOWED read-only — the
    // sidecar synthesizes them from the source's real catalog, because every OpenAI-schooled model
    // probes them first and fighting that instinct just burned agent turns (see
    // debugging/agent/federation-information-schema-probe-cryptic-plan-error.md — superseded by the
    // read-only catalog). `pg_catalog` stays unreachable; steer to the supported catalog instead.
    if upper.contains("PG_CATALOG.") {
        return Err(FederationError::BadSql(
            "pg_catalog is not queryable through federation.query; query \
             information_schema.tables / information_schema.columns instead (read-only), or call \
             the `federation.schema` tool — {source} lists the source's tables, {source, table} \
             lists a table's columns"
                .into(),
        ));
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

    /// A leading `--`/`/* */` comment is legitimate SQL (agents write `-- header` lines); the
    /// keyword gate must look at the first real token, not the comment. A comment must NOT hide
    /// a write, and a JOIN SELECT passes (regression: reported as "JOIN isn't allowed" when the
    /// statement carried a leading comment).
    #[test]
    fn skips_leading_comments_but_still_gates_the_real_leader() {
        assert!(validate_select_host("-- top sites\nSELECT * FROM t").is_ok());
        assert!(validate_select_host("/* header */ SELECT * FROM t").is_ok());
        assert!(
            validate_select_host("-- a\n-- b\n/* c */\nWITH x AS (SELECT 1) SELECT * FROM x")
                .is_ok()
        );
        assert!(validate_select_host(
            "-- joined\nSELECT * FROM \"site\" INNER JOIN \"site_tag\" ON \"site\".\"id\" = \"site_tag\".\"site_id\""
        )
        .is_ok());
        assert!(validate_select_host("-- sneaky\nDROP TABLE t").is_err());
        assert!(validate_select_host("/* only a comment */").is_err());
        assert!(validate_select_host("/* unterminated").is_err());
    }

    #[test]
    fn rejects_multi_statement() {
        assert!(validate_select_host("SELECT 1; DROP TABLE t").is_err());
    }

    /// `information_schema.tables`/`columns` probes pass the host gate — the sidecar answers them
    /// read-only from the source's real catalog (every OpenAI-schooled model probes them first).
    #[test]
    fn allows_information_schema_probes() {
        for s in [
            "SELECT table_name FROM information_schema.tables",
            "select * from INFORMATION_SCHEMA.columns where table_name = 't'",
        ] {
            assert!(validate_select_host(s).is_ok(), "should allow: {s}");
        }
    }

    /// `pg_catalog` stays unreachable — rejected with a message steering to the supported
    /// `information_schema` tables and the `federation.schema` verb.
    #[test]
    fn rejects_pg_catalog_with_steering_message() {
        match validate_select_host("SELECT relname FROM pg_catalog.pg_class") {
            Err(FederationError::BadSql(m)) => {
                assert!(
                    m.contains("information_schema.tables") && m.contains("federation.schema"),
                    "no steer in: {m}"
                )
            }
            other => panic!("should reject pg_catalog: {other:?}"),
        }
    }
}
