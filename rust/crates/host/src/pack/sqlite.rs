//! Materialize a pack's sqlite datasource — the ONE place a pack touches this node's filesystem.
//!
//! ## Why this file exists, and the tradeoff it encodes
//!
//! A pack ships `schema.sql`/`seed.sql` as authored text. The federation `Source` trait deliberately
//! will not run caller SQL (`apply_ddl` takes the migrate planner's allow-listed statements;
//! `write_rows` takes structured rows) — that refusal is correct for a verb a tenant drives against
//! a remote engine they do not own. But a pack's whole promise is "blank node + one call = a working
//! product", and that requires standing a datasource UP, not just registering a pointer to one.
//!
//! The resolution, scoped to the narrowest thing that works: a pack may materialize a **node-local
//! sqlite file** it owns, and only that. The SQL runs in-process (bundled `rusqlite`), never through
//! a shell — the prototype shelled to the `sqlite3` CLI, which this port drops. Any other engine
//! REGISTERS ONLY; the linter warns the author that their SQL will not run.
//!
//! What this costs, stated plainly: the datasource object is the one part of an apply that is not
//! pure bundle-over-the-wire — it writes a file under `{LB_DIR|.lazybones}/packs/`. Every other
//! object kind (rules, dashboards, channels, agent context) is filesystem-free, so a third party
//! applying a pack with no `datasource` block needs nothing but a session and caps. Widening this to
//! a general per-source `exec_sql` seam is a federation-scope question, deliberately NOT decided
//! here.
//!
//! The path is derived from `(pack, datasource)` the way `ext/install_dir.rs` derives a native
//! extension's home — same sanitizer, same escape-proofing, same determinism.

use std::path::PathBuf;

use super::error::PackError;

/// This node's db path for a pack's datasource: `{LB_DIR|.lazybones}/packs/{ws}/{pack}/{name}.db`.
/// Deterministic, and every component is sanitized so an exotic id can never escape the base dir.
pub fn db_path(ws: &str, pack: &str, datasource: &str) -> PathBuf {
    let base = std::env::var("LB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".lazybones"));
    base.join("packs")
        .join(sanitize_component(ws))
        .join(sanitize_component(pack))
        .join(format!("{}.db", sanitize_component(datasource)))
}

/// Build the db file fresh and run `schema` then `seed` into it. Returns the absolute path — the
/// DSN the datasource registers under.
///
/// **Fresh by design:** the file is removed first, so a re-apply rebuilds the dataset rather than
/// layering a second seed onto a stale one. That is what makes "apply twice, same end state" true
/// for the data half, exactly as it is for every other object kind.
pub fn materialize(
    ws: &str,
    pack: &str,
    datasource: &str,
    schema: Option<&str>,
    seed: Option<&str>,
) -> Result<PathBuf, PackError> {
    let path = db_path(ws, pack, datasource);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| PackError::Internal(format!("creating pack data dir: {e}")))?;
    }
    if path.exists() {
        std::fs::remove_file(&path)
            .map_err(|e| PackError::Internal(format!("clearing pack db: {e}")))?;
    }

    let conn = rusqlite::Connection::open(&path)
        .map_err(|e| PackError::Internal(format!("opening pack db: {e}")))?;
    for sql in [schema, seed].into_iter().flatten() {
        // `execute_batch` runs the authored statements as one batch. A syntax error names the
        // statement, which is what a pack author needs to fix their SQL.
        conn.execute_batch(sql)
            .map_err(|e| PackError::BadInput(format!("pack SQL failed: {e}")))?;
    }
    drop(conn);

    path.canonicalize()
        .map_err(|e| PackError::Internal(format!("resolving pack db path: {e}")))
}

fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile pack or datasource id cannot climb out of the base dir — the sanitizer is the only
    /// thing between an authored id and an arbitrary write path.
    #[test]
    fn a_component_can_never_escape_the_base_dir() {
        let p = db_path("../../etc", "../../..", "../../../tmp/evil");
        let s = p.to_string_lossy();
        assert!(!s.contains(".."), "escaped the base dir: {s}");
        assert!(s.contains("packs"), "not under the packs base: {s}");
    }

    /// The workspace wall is structural in the path too — two workspaces never share a db file.
    #[test]
    fn the_path_is_deterministic_and_workspace_scoped() {
        assert_eq!(db_path("a", "bas", "d"), db_path("a", "bas", "d"));
        assert_ne!(db_path("a", "bas", "d"), db_path("b", "bas", "d"));
    }
}
