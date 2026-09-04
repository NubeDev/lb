//! What `store.query` checks before it runs — and, more importantly, what it no longer has to.
//!
//! # The change
//!
//! Under SurrealDB 2 this file WAS the read-only gate: parse the caller's SQL with SurrealDB's own
//! parser and allow only a single `SELECT`/`INFO`/`SHOW`. SurrealDB 3 sealed the judgement that gate
//! read. Checked exhaustively against `surrealdb-core` 3.2.4:
//!
//!   * `syn::parse(&str) -> Ast` is public, but `Ast`'s entire public API is `num_statements`,
//!     `is_value_expression`, `get_let_statements`, `add_param`. Its `expressions` field is
//!     `pub(crate)` (`sql/ast.rs:40`).
//!   * `impl From<Ast> for expr::LogicalPlan` is public (`sql/ast.rs:144`), but `LogicalPlan` and
//!     `TopLevelExpr` are `pub(crate)` (`expr/mod.rs:100`), so the impl cannot be named.
//!   * `read_only()` is `pub(crate)` at all three levels: `TopLevelExpr` (`expr/plan.rs:27`),
//!     `Expr` (`expr/expression.rs:103`), `Block` (`expr/block.rs:70`).
//!
//! So the read-only property is no longer decided here. It is decided by the ENGINE: the statement
//! runs in a session authenticated as a `VIEWER` on the caller's own workspace database, which
//! cannot write and cannot reach another workspace (`lb_store`'s `reader.rs`, and the measurements
//! in `store/tests/viewer_session_probe.rs` and `viewer_namespace_probe.rs`).
//!
//! That is **stronger** than the allowlist it replaces. The allowlist had to be updated by hand
//! whenever SurrealDB added a statement kind, so it could silently drift from the grammar that
//! actually executes. A session that lacks the privilege cannot drift.
//!
//! # What is left in this file, and what each part is for
//!
//! Two things, and it matters which is which:
//!
//!   1. [`ReadKind`] — **bounding, not safety.** `run.rs` wraps a `SELECT` in a bounded sub-select
//!      to apply the row cap and timeout; `INFO`/`SHOW` cannot be subqueried and are inherently one
//!      row. Classifying by leading keyword is fine for that: a misclassification costs a query that
//!      fails to parse when wrapped, never an unsafe execution.
//!   2. [`write_advisory`] — **ergonomics, not safety.** A write refused by the engine comes back as
//!      `Ok([])`, an empty result rather than an error (measured). A caller who sent an `UPDATE`
//!      deserves to be told, not left reading zero rows and guessing. If this misses a write kind
//!      the engine still refuses it, so the failure mode is a worse message, never a mutation.
//!
//! The one genuinely load-bearing check that remains here is the secret-plane wall, and it lives in
//! `secret_wall.rs` because it is a different responsibility with a different argument for why a
//! token scan is sound. Read that file before changing it: a `VIEWER` **can** read the secret
//! tables, so the engine does not cover this one.

use super::error::StoreQueryError;
use super::secret_wall::{ensure_no_secret_tables, Vars};

/// Which shape a statement has, for **bounding** purposes only (see the module note).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadKind {
    /// Wrapped by `run.rs` in a bounded sub-select to apply the row cap and timeout.
    Select,
    /// `INFO` / `SHOW` — one structured row, cannot be subqueried, runs as written.
    Introspection,
}

/// Statement keywords that mutate data or schema. Used only to produce a clear message; the engine
/// is what actually refuses them.
const WRITE_KEYWORDS: &[&str] = &[
    "create", "update", "upsert", "delete", "remove", "insert", "relate", "define", "alter",
    "rebuild", "begin", "commit", "cancel", "use", "let", "sleep", "kill", "live",
];

/// Check a caller-supplied statement and report its shape.
///
/// Refuses only two things: a statement that names the secret plane, and one whose leading keyword
/// is a write. Everything else is allowed through to a session that cannot do damage.
pub fn ensure_read_only(sql: &str) -> Result<ReadKind, StoreQueryError> {
    ensure_read_only_with_vars(sql, &[])
}

/// [`ensure_read_only`] with the caller's bindings, which the secret wall also inspects.
pub fn ensure_read_only_with_vars(sql: &str, vars: Vars<'_>) -> Result<ReadKind, StoreQueryError> {
    if sql.trim().is_empty() {
        return Err(StoreQueryError::Rejected("empty statement".into()));
    }
    // The load-bearing check. Runs FIRST: a statement naming the secret plane is refused whatever
    // else it is.
    ensure_no_secret_tables(sql, vars)?;
    if let Some(kw) = write_advisory(sql) {
        return Err(StoreQueryError::Rejected(format!(
            "`{kw}` writes, and store.query runs on a read-only session. Use the typed verbs \
             (ingest.write, template.save, …) to change data."
        )));
    }
    Ok(kind_of(sql))
}

/// The first write keyword among ALL the statements in `sql`, for the caller-facing message.
///
/// Every statement is checked, not just the first: `SELECT 1; DELETE person` opens with a read, and
/// a caller who wrote that deserves the same message as one who wrote the `DELETE` alone. The engine
/// refuses the write either way — this only decides what the author is told.
fn write_advisory(sql: &str) -> Option<&'static str> {
    statements(sql).into_iter().find_map(|st| {
        let kw = leading_keyword(&st)?;
        WRITE_KEYWORDS.iter().find(|k| **k == kw.as_str()).copied()
    })
}

/// Split on top-level `;`, ignoring semicolons inside quoted strings.
fn statements(sql: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == '\\' {
                    if let Some(n) = chars.next() {
                        cur.push(n);
                    }
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                cur.push(c);
            }
            None if c == ';' => out.push(std::mem::take(&mut cur)),
            None => cur.push(c),
        }
    }
    out.push(cur);
    out.into_iter().filter(|s| !s.trim().is_empty()).collect()
}

/// `SELECT` unless the statement opens with `INFO` or `SHOW`.
fn kind_of(sql: &str) -> ReadKind {
    match leading_keyword(sql).as_deref() {
        Some("info") | Some("show") => ReadKind::Introspection,
        _ => ReadKind::Select,
    }
}

/// The first word of the statement, lowercased, skipping leading whitespace, `(`, and comments.
fn leading_keyword(sql: &str) -> Option<String> {
    let mut rest = sql.trim_start();
    loop {
        if let Some(r) = rest.strip_prefix("--").or_else(|| rest.strip_prefix('#')) {
            rest = r.split_once('\n').map_or("", |(_, t)| t).trim_start();
            continue;
        }
        if let Some(r) = rest.strip_prefix("/*") {
            rest = r.split_once("*/").map_or("", |(_, t)| t).trim_start();
            continue;
        }
        if let Some(r) = rest.strip_prefix('(') {
            rest = r.trim_start();
            continue;
        }
        break;
    }
    let word: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    (!word.is_empty()).then(|| word.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_select_is_allowed_and_classified_for_bounding() {
        assert_eq!(
            ensure_read_only("SELECT * FROM series").unwrap(),
            ReadKind::Select
        );
        assert_eq!(
            ensure_read_only("  (SELECT * FROM series)").unwrap(),
            ReadKind::Select
        );
    }

    #[test]
    fn introspection_is_classified_so_it_is_not_wrapped() {
        for sql in [
            "INFO FOR DB",
            "info for db",
            "SHOW CHANGES FOR TABLE series SINCE 0",
        ] {
            assert_eq!(
                ensure_read_only(sql).unwrap(),
                ReadKind::Introspection,
                "{sql}"
            );
        }
    }

    #[test]
    fn a_write_gets_a_clear_message_rather_than_an_empty_result() {
        for sql in [
            "CREATE person SET a = 1",
            "DELETE person",
            "USE NS other DB main",
        ] {
            let err = ensure_read_only(sql).unwrap_err();
            assert!(
                matches!(err, StoreQueryError::Rejected(_)),
                "{sql} -> {err:?}"
            );
        }
    }

    #[test]
    fn the_secret_wall_runs_before_anything_else() {
        let err = ensure_read_only("SELECT * FROM secret").unwrap_err();
        assert!(
            matches!(err, StoreQueryError::SecretTable("secret")),
            "{err:?}"
        );
        // …including when the statement is also a write: the secret refusal is the one reported.
        let err = ensure_read_only("DELETE secret").unwrap_err();
        assert!(
            matches!(err, StoreQueryError::SecretTable("secret")),
            "{err:?}"
        );
    }

    /// A write hidden behind a leading read still gets the message.
    #[test]
    fn a_write_after_a_read_is_still_advised() {
        assert!(ensure_read_only("SELECT * FROM series; DELETE series").is_err());
        assert!(ensure_read_only("SELECT 1; USE NS other DB main").is_err());
        // …and a semicolon inside a string is not a statement break.
        assert_eq!(
            ensure_read_only("SELECT * FROM series WHERE name = 'a; DELETE b'").unwrap(),
            ReadKind::Select
        );
    }

    #[test]
    fn an_empty_statement_is_refused() {
        assert!(ensure_read_only("   ").is_err());
    }

    /// A comment before the keyword must not hide it.
    #[test]
    fn a_leading_comment_does_not_hide_the_keyword() {
        assert!(ensure_read_only("-- harmless\nDELETE person").is_err());
        assert!(ensure_read_only("/* x */ CREATE person SET a = 1").is_err());
    }
}
