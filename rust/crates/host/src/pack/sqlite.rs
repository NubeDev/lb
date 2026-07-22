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

/// Resolve an already-materialized pack db WITHOUT touching its rows — the re-apply path (seed
/// ownership, `pack-entity-binding-scope.md`). `Ok(Some(path))` when the file exists (re-register
/// against the operator's live data); `Ok(None)` when it is absent, so the caller rebuilds it fresh.
/// A missing db is not an error: the seed is starting data, and with no file there is no operator
/// data to protect — rebuilding reaches the same end state as a first apply (idempotence holds).
pub fn resolve_existing(
    ws: &str,
    pack: &str,
    datasource: &str,
) -> Result<Option<PathBuf>, PackError> {
    let path = db_path(ws, pack, datasource);
    if !path.is_file() {
        return Ok(None);
    }
    path.canonicalize()
        .map(Some)
        .map_err(|e| PackError::Internal(format!("resolving pack db path: {e}")))
}

/// Reconcile an existing pack db's schema up to `schema_sql` ADDITIVELY — the UPGRADE path
/// (`pack-upgrade-scope.md`). For each `CREATE TABLE` the pack declares: a table the db LACKS is
/// created (its full authored statement runs); a table that EXISTS gains any column it lacks via
/// `ALTER TABLE ADD COLUMN <col def>`. Returns the human labels of what it added (for the loud
/// upgrade note). Operator rows are untouched — a new column is nullable/empty on existing rows.
///
/// ADDITIVE ONLY (v1, deliberately — `pack-upgrade-scope.md` §Non-goals): a column the pack REMOVED
/// is left in place (the safe direction), a retyped column is not altered, and no data is backfilled.
/// A destructive migration is an explicit future act, never an upgrade side effect. sqlite's
/// `ALTER TABLE ADD COLUMN` cannot add a NOT NULL column without a default — such a column is skipped
/// with a note rather than failing the whole upgrade (the pack must ship it nullable or with a default
/// to add it to a populated table; a fresh apply gets the strict shape).
pub fn reconcile_schema(
    path: &std::path::Path,
    schema_sql: &str,
) -> Result<Vec<String>, PackError> {
    let tables = parse_create_tables(schema_sql);
    let conn = rusqlite::Connection::open(path)
        .map_err(|e| PackError::Internal(format!("opening pack db for reconcile: {e}")))?;
    let mut added: Vec<String> = Vec::new();

    for t in &tables {
        if !table_exists(&conn, &t.name) {
            // A brand-new table: run its authored CREATE verbatim (no rows to protect).
            conn.execute_batch(&t.create_sql)
                .map_err(|e| PackError::BadInput(format!("creating table '{}': {e}", t.name)))?;
            added.push(format!("table {}", t.name));
            continue;
        }
        let live: std::collections::BTreeSet<String> = live_columns(&conn, &t.name)?
            .into_iter()
            .map(|c| c.to_lowercase())
            .collect();
        for col in &t.columns {
            if live.contains(&col.name.to_lowercase()) {
                continue;
            }
            // sqlite refuses ADD COLUMN of a NOT NULL column with no DEFAULT on a populated table.
            let notnull = col.def.to_lowercase().contains("not null");
            let has_default = col.def.to_lowercase().contains("default");
            if notnull && !has_default {
                return Ok({
                    added.push(format!(
                        "column {}.{} SKIPPED (NOT NULL without a default cannot be added to \
                         existing rows — ship it nullable or with a default)",
                        t.name, col.name
                    ));
                    added
                });
            }
            let sql = format!(
                "ALTER TABLE {} ADD COLUMN {}",
                quote_ident(&t.name),
                col.def
            );
            conn.execute(&sql, [])
                .map_err(|e| PackError::BadInput(format!("adding {}.{}: {e}", t.name, col.name)))?;
            added.push(format!("column {}.{}", t.name, col.name));
        }
    }
    Ok(added)
}

/// Double-quote a sqlite identifier (embedded `"` doubled) — the table name in the generated ALTER.
fn quote_ident(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn table_exists(conn: &rusqlite::Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |_| Ok(()),
    )
    .is_ok()
}

fn live_columns(conn: &rusqlite::Connection, table: &str) -> Result<Vec<String>, PackError> {
    let mut stmt = conn
        .prepare("SELECT name FROM pragma_table_info(?1)")
        .map_err(|e| PackError::Internal(format!("reading columns of '{table}': {e}")))?;
    let rows = stmt
        .query_map([table], |r| r.get::<_, String>(0))
        .map_err(|e| PackError::Internal(format!("reading columns of '{table}': {e}")))?;
    Ok(rows.filter_map(Result::ok).collect())
}

/// One parsed `CREATE TABLE` the reconcile works from: the table name, its full authored statement
/// (to create the table wholesale when absent), and each top-level column (name + its raw definition
/// text, so a missing column can be re-emitted verbatim in an `ADD COLUMN`).
struct ParsedTable {
    name: String,
    create_sql: String,
    columns: Vec<ParsedColumn>,
}
struct ParsedColumn {
    name: String,
    /// The column's definition as authored — e.g. `lat REAL` or `name TEXT NOT NULL`. Emitted after
    /// `ADD COLUMN` verbatim, so the added column matches the pack's declared type/constraints.
    def: String,
}

/// Parse the `CREATE TABLE <name> ( <col defs> )` statements out of a pack's schema SQL. Dialect-blind
/// and scoped to the subset packs ship: it finds each `create table`, reads the name, and splits the
/// paren body on top-level commas into column definitions (a table-level constraint like
/// `PRIMARY KEY (a, b)` leads with a keyword and is skipped — its leading token is not a bare column,
/// which the caller's live-column check simply never matches). A statement it cannot bound is skipped
/// (the table stays unreconciled — a fresh apply is the fallback), never guessed.
fn parse_create_tables(schema_sql: &str) -> Vec<ParsedTable> {
    let bytes = schema_sql.as_bytes();
    let lower = schema_sql.to_lowercase();
    let mut out = Vec::new();
    let mut search = 0usize;
    while let Some(rel) = lower[search..].find("create table") {
        let kw = search + rel;
        let after_kw = kw + "create table".len();
        let mut i = skip_ws(bytes, after_kw);
        if lower[i..].starts_with("if not exists") {
            i = skip_ws(bytes, i + "if not exists".len());
        }
        let (name, after_name) = read_ident(bytes, i);
        search = after_name.max(after_kw + 1);
        let Some(name) = name else { continue };
        let Some(open) = lower[after_name..].find('(').map(|p| after_name + p) else {
            continue;
        };
        let Some(close) = match_paren(bytes, open) else {
            continue;
        };
        // The full authored statement, `(`-to-`)` inclusive plus the `CREATE TABLE …` head, terminated
        // with a `;` so `execute_batch` runs exactly it.
        let create_sql = format!("{};", &schema_sql[kw..=close]);
        let columns = parse_columns(&schema_sql[open + 1..close]);
        out.push(ParsedTable {
            name,
            create_sql,
            columns,
        });
        search = close + 1;
    }
    out
}

/// Split a `CREATE TABLE` paren body into top-level column definitions (name + raw def text). Items
/// whose leading token is a table-constraint keyword (`primary`/`foreign`/`unique`/`check`/`constraint`)
/// are dropped — they are not columns.
fn parse_columns(body: &str) -> Vec<ParsedColumn> {
    let b = body.as_bytes();
    let mut cols = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut i = 0usize;
    let push = |slice: &str, cols: &mut Vec<ParsedColumn>| {
        let def = slice.trim();
        if def.is_empty() {
            return;
        }
        let (ident, _) = read_ident(def.as_bytes(), 0);
        let Some(name) = ident else { return };
        let lname = name.to_lowercase();
        if matches!(
            lname.as_str(),
            "primary" | "foreign" | "unique" | "check" | "constraint"
        ) {
            return; // a table-level constraint, not a column
        }
        cols.push(ParsedColumn {
            name,
            def: def.to_string(),
        });
    };
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                push(&body[start..i], &mut cols);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    push(&body[start..], &mut cols);
    cols
}

// ----- tiny scanning helpers (shared shape with lb-packs `binding.rs`) --------------------------

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn read_ident(b: &[u8], i: usize) -> (Option<String>, usize) {
    let i = skip_ws(b, i);
    if i >= b.len() {
        return (None, i);
    }
    if b[i] == b'"' {
        let mut j = i + 1;
        while j < b.len() && b[j] != b'"' {
            j += 1;
        }
        let name: String = b[i + 1..j.min(b.len())]
            .iter()
            .map(|&c| c as char)
            .collect();
        return (Some(name), (j + 1).min(b.len()));
    }
    let start = i;
    let mut j = i;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    if j == start {
        return (None, i);
    }
    (Some(b[start..j].iter().map(|&c| c as char).collect()), j)
}

fn match_paren(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
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

    // ----- pack UPGRADE: schema reconciliation (pack-upgrade-scope) --------------------------------

    #[test]
    fn parse_reads_tables_columns_and_the_full_create() {
        let ts = parse_create_tables(
            "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL, lat REAL, lng REAL);\
             CREATE TABLE tag (site_id TEXT, tag TEXT, PRIMARY KEY (site_id, tag));",
        );
        assert_eq!(ts.len(), 2);
        assert_eq!(ts[0].name, "site");
        let cols: Vec<&str> = ts[0].columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(cols, vec!["id", "name", "lat", "lng"]);
        // The table-level PRIMARY KEY(...) is NOT read as a column.
        let tagcols: Vec<&str> = ts[1].columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(tagcols, vec!["site_id", "tag"]);
        assert!(ts[0].create_sql.starts_with("CREATE TABLE site"));
    }

    /// The load-bearing test: an existing db with the OLD schema + a row, reconciled to a schema that
    /// adds two nullable columns → the columns exist, the row SURVIVED, the new columns are NULL on it.
    #[test]
    fn reconcile_adds_nullable_columns_and_preserves_rows() {
        let path = std::env::temp_dir().join(format!("lb-reconcile-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL);")
                .unwrap();
            c.execute("INSERT INTO site VALUES ('s1','One')", [])
                .unwrap();
        }
        let added = reconcile_schema(
            &path,
            "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL, lat REAL, lng REAL);",
        )
        .unwrap();
        assert!(added.iter().any(|a| a.contains("site.lat")), "{added:?}");
        assert!(added.iter().any(|a| a.contains("site.lng")), "{added:?}");
        let c = rusqlite::Connection::open(&path).unwrap();
        // The row survived, and the new column is NULL on it.
        let (name, lat): (String, Option<f64>) = c
            .query_row("SELECT name, lat FROM site WHERE id='s1'", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!(name, "One");
        assert_eq!(lat, None);
        let _ = std::fs::remove_file(&path);
    }

    /// A brand-new table (no prior existence, no rows to protect) is created wholesale on reconcile.
    #[test]
    fn reconcile_creates_a_brand_new_table() {
        let path = std::env::temp_dir().join(format!("lb-reconcile-new-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE site (id TEXT PRIMARY KEY);")
                .unwrap();
        }
        let added = reconcile_schema(
            &path,
            "CREATE TABLE site (id TEXT PRIMARY KEY);\
             CREATE TABLE note (id TEXT PRIMARY KEY, body TEXT);",
        )
        .unwrap();
        assert!(added.iter().any(|a| a.contains("table note")), "{added:?}");
        let c = rusqlite::Connection::open(&path).unwrap();
        assert!(super::table_exists(&c, "note"));
        let _ = std::fs::remove_file(&path);
    }

    /// Reconcile is a NO-OP when the live schema already matches (idempotent — an upgrade re-run adds
    /// nothing).
    #[test]
    fn reconcile_is_a_noop_when_nothing_is_missing() {
        let path =
            std::env::temp_dir().join(format!("lb-reconcile-noop-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT);")
                .unwrap();
        }
        let added =
            reconcile_schema(&path, "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT);").unwrap();
        assert!(added.is_empty(), "{added:?}");
        let _ = std::fs::remove_file(&path);
    }

    /// ADDITIVE ONLY: a column the pack REMOVED is left in place (the safe direction), not dropped.
    #[test]
    fn reconcile_never_drops_a_removed_column() {
        let path =
            std::env::temp_dir().join(format!("lb-reconcile-drop-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        {
            let c = rusqlite::Connection::open(&path).unwrap();
            c.execute_batch("CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT, legacy TEXT);")
                .unwrap();
        }
        // The new schema omits `legacy` — reconcile must NOT drop it.
        let added =
            reconcile_schema(&path, "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT);").unwrap();
        assert!(added.is_empty());
        let c = rusqlite::Connection::open(&path).unwrap();
        assert!(
            live_columns(&c, "site")
                .unwrap()
                .iter()
                .any(|c| c == "legacy"),
            "the removed column must survive (additive-only)"
        );
        let _ = std::fs::remove_file(&path);
    }
}
