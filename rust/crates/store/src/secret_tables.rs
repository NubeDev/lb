//! The canonical **secret-plane table set** — the one list every read/copy surface refuses, and the
//! only place it is written down. It was born inside `snapshot_guard` (a snapshot must be
//! structurally incapable of carrying credential material); the node-update scope (decision 9, "the
//! raw-read wall") made it load-bearing for *reads* as well, so it lives here and both consumers —
//! `snapshot_guard` and the host's raw-read surfaces (`store.query`, `store.scan`, `store.graph`) —
//! read the SAME slice. A second list would be the bug: one of them would drift, and the drift would
//! be a credential leak nobody notices.
//!
//! **Why a table list and not a capability.** The secret plane has an owner gate on `secret.get`, but
//! a raw read over the store (a SELECT, a scan, a graph walk) never passes through it. The refusal
//! here is therefore **independent of capability grants and of principal** — a workspace admin, an
//! extension and the host's own MCP surfaces hit it identically (rule 10: no caller is special-cased,
//! and there is no override cap). Widening the list is a deliberate act with a test.
//!
//! Matching is **case-insensitive**, deliberately wider than SurrealDB's own case-sensitive table
//! naming: `SELECT * FROM SECRET` reads an unrelated (empty) table in SurrealDB, so refusing it is a
//! false refusal — a cheap, visible one — where matching case-sensitively would leave a rename-shaped
//! trap for whoever next adds a table.

/// Every table that holds credential material. Refused by the snapshot guard (never copied) and by
/// the raw-read wall (never read back), whatever a caller's plan or capability says.
pub const SECRET_TABLES: &[&str] = &[
    // lb-secrets: `secret:{ws}:{path}` — plaintext-in-store today (envelope encryption is its own
    // stage), so any read is a verbatim disclosure of the credential.
    "secret",
    // Per-workspace credential records (login-hardening scope) — argon2 hashes.
    "credential",
    // Global password records (email-login scope) — argon2 hashes.
    "identity_credential",
    // API-key records — token material.
    "apikey",
];

/// Is `table` part of the secret plane? Case-insensitive (see the module note) and whitespace-trimmed
/// — a table name that only *renders* differently must not slip through.
pub fn is_secret_table(table: &str) -> bool {
    let t = table.trim();
    SECRET_TABLES.iter().any(|s| t.eq_ignore_ascii_case(s))
}

/// The matching secret-plane table name for `table`, if any — for a refusal message that names the
/// table the caller actually asked for.
pub fn secret_table_of(table: &str) -> Option<&'static str> {
    let t = table.trim();
    SECRET_TABLES
        .iter()
        .find(|s| t.eq_ignore_ascii_case(s))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_plane_hits_and_ordinary_tables_miss() {
        for t in ["secret", "credential", "identity_credential", "apikey"] {
            assert!(is_secret_table(t), "{t} is the secret plane");
        }
        for t in ["series", "dashboard", "site", "secrets", "my_secret", "sec"] {
            assert!(!is_secret_table(t), "{t} must NOT be refused");
        }
    }

    #[test]
    fn matching_ignores_case_and_padding() {
        for t in ["SECRET", "Secret", " secret ", "ApiKey"] {
            assert!(is_secret_table(t), "{t} must be refused");
        }
        assert_eq!(secret_table_of("SECRET"), Some("secret"));
        assert_eq!(secret_table_of("dashboard"), None);
    }

    /// The list is duplicate-free and part of the reserved (host-owned) set — a secret table that is
    /// not reserved would be writable through the generic store CRUD surface.
    #[test]
    fn every_secret_table_is_reserved_and_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for t in SECRET_TABLES {
            assert!(seen.insert(*t), "duplicate secret table: {t}");
            assert!(
                crate::reserved::is_reserved(t),
                "secret table {t} must also be host-owned/reserved"
            );
        }
    }
}
