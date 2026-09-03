//! The **secret-plane wall** for `store.query` — no statement may name a credential table.
//!
//! # Why this is a token scan and not an AST walk
//!
//! It used to walk the parsed statement and check each table position. SurrealDB 3 sealed the AST
//! (`parse.rs` records the exhaustive check), so the table positions are no longer visible to us.
//!
//! The read/write half of the old gate did not need replacing in kind — the ENGINE now refuses
//! writes, in a session that cannot perform them (`lb_store`'s `reader.rs`). That substitution is
//! not available here: a `VIEWER` at database level **bypasses table permissions for reads**
//! (`ctx::check_perms`, measured in `store/tests/viewer_reads_secrets_probe.rs`), so the engine will
//! happily read `secret` for us. The wall has to stay host-side.
//!
//! # Why a token scan is sound HERE, where it would not be for writes
//!
//! The two walls fail in opposite directions, and that is the whole argument:
//!
//!   * A text check for **writes** that misses one silently executes a mutation. Unsafe.
//!   * A text check for **secret tables** that misses one would disclose a credential — so it is
//!     built to over-match instead: any identifier token equal to a secret table name refuses the
//!     whole statement, wherever it appears. A refused `SELECT note FROM log WHERE kind = apikey`
//!     is a visible false refusal; there is no silent failure mode.
//!
//! Tokens are compared with [`lb_store::secret_tables::is_secret_table`] — the one canonical list,
//! never a second copy. Matching is on whole identifiers, so `secrets`, `my_secret` and `sec` are
//! not refused, exactly as that module's own tests require.
//!
//! # The one hole, closed explicitly
//!
//! A token scan cannot see a table named *indirectly* — `FROM type::table($t)` builds the name at
//! run time from a value. The old AST wall resolved that against the bindings. We cannot, so
//! dynamic table construction is **refused outright** ([`DYNAMIC_TABLE_FNS`]). That narrows the verb
//! (a parameterised table position used to be allowed) and is the honest trade: a narrower verb
//! beats a wall with a documented way through it.

use lb_store::secret_tables::secret_table_of;
use serde_json::Value;

use super::error::StoreQueryError;
use super::sql_scan::{
    dynamic_table_args, from_terms, identifiers, resolve_arg, strip_comments,
    table_term_is_provable,
};

pub(crate) type Vars<'a> = &'a [(String, Value)];

/// Refuse `sql` if it names a secret-plane table, or if it could name one indirectly.
///
/// `vars` are checked too: a binding whose *value* is a secret table name is refused, so a caller
/// cannot move the name out of the statement text and into a parameter.
pub fn ensure_no_secret_tables(sql: &str, vars: Vars<'_>) -> Result<(), StoreQueryError> {
    let stripped = strip_comments(sql);
    // A dynamically-named table is resolved to the name it will actually take, so an innocent
    // parameterised table position still works; one that cannot be resolved is refused.
    for arg in dynamic_table_args(&stripped) {
        match resolve_arg(&arg, vars) {
            Some(name) => {
                if let Some(t) = secret_table_of(&name) {
                    return Err(StoreQueryError::SecretTable(t));
                }
            }
            None => {
                return Err(StoreQueryError::Rejected(format!(
                    "the table name in `{arg}` is built at run time and cannot be checked against \
                     the secret plane. Name the table directly, or bind it to a literal."
                )))
            }
        }
    }
    // A table position that is COMPUTED — `FROM some.field` — names a table we cannot know. The
    // token scan would read `some` and `field` as ordinary identifiers and let it through, so the
    // term after each `FROM` is judged on its own.
    for term in from_terms(&stripped) {
        if !table_term_is_provable(&term, vars) {
            return Err(StoreQueryError::Rejected(format!(
                "the table in `FROM {term}` is chosen at run time and cannot be checked against \
                 the secret plane. Name the table directly, or bind it to a literal."
            )));
        }
    }
    for token in identifiers(&stripped) {
        if let Some(t) = secret_table_of(&token) {
            return Err(StoreQueryError::SecretTable(t));
        }
    }
    for (_, v) in vars {
        if let Some(s) = v.as_str() {
            if let Some(t) = secret_table_of(s) {
                return Err(StoreQueryError::SecretTable(t));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(sql: &str) -> bool {
        ensure_no_secret_tables(sql, &[]).is_err()
    }

    #[test]
    fn a_direct_secret_read_is_refused() {
        for sql in [
            "SELECT * FROM secret",
            "SELECT * FROM credential",
            "SELECT * FROM identity_credential",
            "SELECT * FROM apikey",
        ] {
            assert!(refused(sql), "{sql} must be refused");
        }
    }

    #[test]
    fn case_backticks_and_nesting_do_not_get_through() {
        for sql in [
            "SELECT * FROM SECRET",
            "SELECT * FROM `secret`",
            "SELECT * FROM ⟨secret⟩",
            "SELECT * FROM (SELECT * FROM secret)",
            "SELECT * FROM ApiKey",
            "SELECT * FROM r\"secret:abc\"",
        ] {
            assert!(refused(sql), "{sql} must be refused");
        }
    }

    #[test]
    fn a_name_split_by_a_comment_does_not_get_through() {
        assert!(refused("SELECT * FROM /* x */ secret"));
        assert!(refused("SELECT * FROM secret -- trailing\n"));
    }

    #[test]
    fn ordinary_tables_are_not_refused() {
        for sql in [
            "SELECT * FROM series",
            "SELECT * FROM secrets",
            "SELECT * FROM my_secret",
            "SELECT * FROM sec",
            "SELECT * FROM dashboard WHERE name = 'apikey rotation'",
        ] {
            assert!(!refused(sql), "{sql} must NOT be refused");
        }
    }

    #[test]
    fn a_dynamic_table_is_resolved_from_its_binding() {
        let secret = vec![("t".to_string(), Value::from("secret"))];
        assert!(matches!(
            ensure_no_secret_tables("SELECT * FROM type::table($t)", &secret),
            Err(StoreQueryError::SecretTable("secret"))
        ));
        let ordinary = vec![("t".to_string(), Value::from("site"))];
        assert!(ensure_no_secret_tables("SELECT * FROM type::table($t)", &ordinary).is_ok());
    }

    /// `type::record` is SurrealDB 3's name for what 2.x called `type::thing`, and lb uses it in
    /// 167 places — so a wall that watched only `type::table`/`type::thing` left the commonest
    /// dynamic form completely unguarded.
    #[test]
    fn the_record_constructor_is_watched_too() {
        assert!(matches!(
            ensure_no_secret_tables("SELECT * FROM type::record('secret', 'x')", &[]),
            Err(StoreQueryError::SecretTable("secret"))
        ));
        let bound = vec![("t".to_string(), Value::from("secret"))];
        assert!(matches!(
            ensure_no_secret_tables("SELECT * FROM type::record($t, 'x')", &bound),
            Err(StoreQueryError::SecretTable("secret"))
        ));
        // …and the innocent form still reads.
        assert!(ensure_no_secret_tables("SELECT * FROM type::record('site', 'x')", &[]).is_ok());
    }

    /// Any OTHER `type::` call in a table position is unprovable, not waved through.
    #[test]
    fn an_unknown_type_constructor_is_not_assumed_safe() {
        // A real `type::` function that is NOT a table constructor: it must not inherit the prefix
        // shortcut just because its path starts with `type::`.
        assert!(refused("SELECT * FROM type::field(meta.tb)"));
    }

    #[test]
    fn a_dynamic_table_literal_is_refused_by_name() {
        assert!(matches!(
            ensure_no_secret_tables("SELECT * FROM type::table('secret')", &[]),
            Err(StoreQueryError::SecretTable("secret"))
        ));
        assert!(ensure_no_secret_tables("SELECT * FROM type::table('site')", &[]).is_ok());
    }

    #[test]
    fn a_computed_table_position_is_refused_but_ordinary_field_reads_are_not() {
        assert!(refused("SELECT * FROM some.field"));
        // A field reference that is NOT a table position must still be fine — this is the shape
        // every composed/bounded subquery produces.
        assert!(!refused(
            "SELECT data.name AS n FROM site WHERE data.id = 'x' ORDER BY data.name"
        ));
        assert!(!refused("SELECT * FROM (SELECT data.name FROM site)"));
        assert!(!refused("SELECT * FROM site"));
        assert!(!refused("SELECT * FROM `site`"));
    }

    /// The case the wall exists for: a table position it cannot prove is refused, not guessed.
    #[test]
    fn an_unprovable_dynamic_table_is_refused() {
        // No binding for `$t`.
        assert!(refused("SELECT * FROM type::table($t)"));
        // An expression, not a literal or a parameter.
        assert!(refused(
            "SELECT * FROM type::table(string::concat('sec','ret'))"
        ));
        assert!(refused("SELECT * FROM type::record(meta.tb, meta.id)"));
    }

    #[test]
    fn a_secret_name_hidden_in_a_binding_is_refused() {
        let vars = vec![("t".to_string(), Value::from("secret"))];
        assert!(ensure_no_secret_tables("SELECT * FROM series", &vars).is_err());
    }
}
