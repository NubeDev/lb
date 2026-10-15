//! A sqlite cell to and from JSON: the direct read path (each cell by its own storage class, no Arrow
//! intermediate) and the write path's parameter binding. Split out of `sqlite.rs`, which owns the pools and the `Source` impl — rendering a
//! cell is its own responsibility (FILE-LAYOUT §9), the sibling of `pg_cells.rs`.
//!
//! Why not Arrow: the default `query_direct_json` goes through `query_direct`'s batches, and
//! `query_arrow` infers each column's type from the FIRST row it reads, so a NULL in row 1 typed the
//! column as null and blanked every value below it (`SELECT NULL UNION ALL SELECT 190.0` →
//! `[null, null]`). sqlite is dynamically typed and the caller wants JSON, so there is no column
//! type to get right in the first place.

use rusqlite::types::ValueRef;
use serde_json::Value;

use super::SourceError;

/// Run `sql` against the sqlite file at `path` and return column-aligned `(columns, rows)`. A
/// short-lived read-only connection, as `SqliteSource::list_columns_with_types` uses — sqlite serves
/// concurrent readers, and rusqlite is blocking, so it runs on `spawn_blocking`.
pub(super) async fn read_json(
    path: &str,
    sql: &str,
) -> Result<(Vec<String>, Vec<Value>), SourceError> {
    let path = path.to_string();
    let sql = sql.to_string();
    tokio::task::spawn_blocking(move || -> Result<_, rusqlite::Error> {
        let conn = rusqlite::Connection::open_with_flags(
            &path,
            rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let mut stmt = conn.prepare(&sql)?;
        let columns: Vec<String> = stmt.column_names().into_iter().map(String::from).collect();
        let width = columns.len();
        let rows = stmt
            .query_map([], |row| {
                (0..width)
                    .map(|i| row.get_ref(i).map(cell))
                    .collect::<Result<Vec<_>, _>>()
                    .map(Value::Array)
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok((columns, rows))
    })
    .await
    .map_err(|e| SourceError(format!("direct worker: {e}")))?
    .map_err(|e| SourceError(format!("direct query: {e}")))
}

/// One sqlite cell as JSON (the read side of [`json_to_sqlite`]). A non-finite REAL has no
/// JSON form and reads as null; a BLOB reads as lowercase hex, as Arrow's binary-to-JSON did.
fn cell(v: ValueRef<'_>) -> Value {
    match v {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => i.into(),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(Value::Null, Value::Number),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(),
        ValueRef::Blob(b) => b
            .iter()
            .map(|x| format!("{x:02x}"))
            .collect::<String>()
            .into(),
    }
}

/// Map a JSON cell value to a sqlite `Value` (parameterized — never inlined). sqlite is dynamically
/// typed; we coerce permissively (a number-looking string stays a string) so the engine's own
/// permissive affinity rules apply at query time.
pub(super) fn json_to_sqlite(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(*b as i64),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Integer(i)
            } else if let Some(f) = n.as_f64() {
                Value::Real(f)
            } else {
                Value::Text(n.to_string())
            }
        }
        serde_json::Value::String(s) => Value::Text(s.clone()),
        // Structured values (arrays/objects) serialize to JSON text (a `json`/`text` column reads
        // them back via `json_extract`). A binary column would want bytes — v1 stores JSON text.
        other => Value::Text(other.to_string()),
    }
}
