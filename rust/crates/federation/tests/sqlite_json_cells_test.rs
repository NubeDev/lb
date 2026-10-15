//! SQLite direct reads to JSON keep every cell (real seeded SQLite file, no mocks — testing-scope §0).
//!
//! The bug this pins: `federation.query` on a sqlite source went through Arrow batches whose column
//! types `query_arrow` inferred from the FIRST row, so a NULL in row 1 blanked the whole column
//! (`SELECT NULL UNION ALL SELECT 190.0` → `[null, null]`). A settings table where one row leaves a
//! column empty and the next fills it read back as all-empty. Route `SqliteSource::query_direct_json`
//! back through the Arrow default and `blank_first_row_does_not_blank_the_column` goes red.

// The crate is a binary, so the test binary composes the modules it needs directly (same pattern as
// `read_concurrency_test.rs`).
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/event.rs"]
mod event;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/info_schema.rs"]
mod info_schema;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/pool.rs"]
mod pool;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/query.rs"]
mod query;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/results.rs"]
mod results;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/source/mod.rs"]
mod source;
#[allow(dead_code)] // shared src module: only part of it is used by this test
#[path = "../src/validate.rs"]
mod validate;

use serde_json::json;
use source::{Source, SqliteSource};

/// A real SQLite file: two settings rows, the first leaving `jan` empty, the second filling it.
fn seed(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("lb-fed-json-{tag}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open sqlite fixture");
    conn.execute_batch(
        "CREATE TABLE setting (id TEXT PRIMARY KEY, value REAL, jan REAL, note TEXT, raw BLOB);
         INSERT INTO setting VALUES ('a-effective-kw', 190, NULL, NULL, NULL);
         INSERT INTO setting VALUES ('b-design', 1023200, 123400.5, 'kWh', x'0aff');",
    )
    .expect("seed");
    path.to_string_lossy().into_owned()
}

#[tokio::test]
async fn blank_first_row_does_not_blank_the_column() {
    let src = SqliteSource::connect(&seed("blank"))
        .await
        .expect("connect");
    let (columns, rows) = src
        .query_direct_json("SELECT id, jan FROM setting ORDER BY id")
        .await
        .expect("query");
    assert_eq!(columns, ["id", "jan"]);
    assert_eq!(
        rows,
        [
            json!(["a-effective-kw", null]),
            json!(["b-design", 123400.5])
        ]
    );

    let (_, rows) = src
        .query_direct_json("SELECT NULL AS v UNION ALL SELECT 190.0")
        .await
        .expect("literal query");
    assert_eq!(rows, [json!([null]), json!([190.0])]);
}

#[tokio::test]
async fn one_column_holds_a_number_and_json_text() {
    // A typed settings column: a number on one row, a JSON document (as text) on the next.
    let src = SqliteSource::connect(&seed("mixed"))
        .await
        .expect("connect");
    let (_, rows) = src
        .query_direct_json(
            "SELECT 190.0 AS value UNION ALL SELECT '{\"jan\":123400,\"feb\":101400}'",
        )
        .await
        .expect("query");
    assert_eq!(
        rows,
        [json!([190.0]), json!(["{\"jan\":123400,\"feb\":101400}"])]
    );
}

#[tokio::test]
async fn cells_keep_their_own_storage_class() {
    let src = SqliteSource::connect(&seed("classes"))
        .await
        .expect("connect");
    let (columns, rows) = src
        .query_direct_json("SELECT id, value, note, raw, 7 AS n FROM setting WHERE id = 'b-design'")
        .await
        .expect("query");
    assert_eq!(columns, ["id", "value", "note", "raw", "n"]);
    assert_eq!(rows, [json!(["b-design", 1023200.0, "kWh", "0aff", 7])]);

    let (columns, rows) = src
        .query_direct_json("SELECT id FROM setting WHERE id = 'missing'")
        .await
        .expect("empty query");
    assert_eq!((columns, rows), (vec!["id".to_string()], vec![]));
}
