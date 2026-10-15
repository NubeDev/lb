//! Entity-scoped writes end to end on a real sqlite file (no mocks): the scoped entry points the
//! dispatcher calls, every attack the scope names, and the unrestricted path unchanged.
//!
//! Mutation check (run 2026-10-15): make `write_sql::upsert`'s guard always true (`WHERE 1=1 OR …`,
//! which keeps its ids bound) and `a_key_held_by_another_entity_is_not_taken_over` and
//! `one_bad_row_writes_nothing` go red. Skipping `check_rows` fails the new-row and move-out cases; the
//! same always-true edit in `write_sql::delete` fails `a_delete_never_reaches_another_entity`.

use serde_json::{json, Value};

use super::write::{delete, write};

fn db(tag: &str) -> String {
    let path = std::env::temp_dir().join(format!("lb-fed-scoped-{tag}-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let conn = rusqlite::Connection::open(&path).expect("open");
    conn.execute_batch(
        "CREATE TABLE setting (id TEXT PRIMARY KEY, site_id TEXT, value REAL);
         INSERT INTO setting VALUES ('a1', 'a', 1), ('b1', 'b', 2);",
    )
    .expect("seed");
    path.to_string_lossy().into_owned()
}

/// The call input a restricted caller reaching site `a` sends (`writable` per the admin's policy).
fn call(writable: bool, ids: &[&str]) -> Value {
    json!({ "write_scope": {
        "policy": {
            "tables": { "setting": { "kind": "entity", "column": "site_id", "writable": writable } },
            "entity_key": { "table": "tags", "columns": ["id"], "key_col": "k", "key_value": "site", "value_col": "v" }
        },
        "ids": ids,
    }})
}

fn rows_of(path: &str) -> Vec<(String, String, f64)> {
    let conn = rusqlite::Connection::open(path).unwrap();
    let mut stmt = conn
        .prepare("SELECT id, site_id, value FROM setting ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn cols() -> Vec<String> {
    ["id", "site_id", "value"].map(String::from).to_vec()
}

async fn save(path: &str, call: &Value, rows: Value) -> Result<u64, String> {
    let key = ["id".to_string()];
    let rows: Vec<Value> = rows.as_array().unwrap().clone();
    write(
        ("sqlite", path, "setting"),
        &cols(),
        &rows,
        Some(&key),
        call,
    )
    .await
}

#[tokio::test]
async fn own_rows_are_written() {
    let p = db("own");
    assert_eq!(
        save(
            &p,
            &call(true, &["a"]),
            json!([["a1", "a", 10.0], ["a2", "a", 3.0]])
        )
        .await,
        Ok(2)
    );
    assert_eq!(
        rows_of(&p)[..2],
        [
            ("a1".into(), "a".into(), 10.0),
            ("a2".into(), "a".into(), 3.0)
        ]
    );
}

#[tokio::test]
async fn a_row_for_another_entity_is_refused() {
    let p = db("other");
    assert!(save(&p, &call(true, &["a"]), json!([["x", "b", 5.0]]))
        .await
        .is_err());
    assert_eq!(rows_of(&p).len(), 2, "nothing written");
}

#[tokio::test]
async fn a_key_held_by_another_entity_is_not_taken_over() {
    let p = db("takeover");
    // `b1` belongs to site b; writing it as site a must not rewrite b's row.
    assert!(save(&p, &call(true, &["a"]), json!([["b1", "a", 99.0]]))
        .await
        .is_err());
    assert!(rows_of(&p).contains(&("b1".into(), "b".into(), 2.0)));
}

#[tokio::test]
async fn an_own_row_cannot_move_out() {
    let p = db("move");
    assert!(save(&p, &call(true, &["a"]), json!([["a1", "b", 1.0]]))
        .await
        .is_err());
    assert!(rows_of(&p).contains(&("a1".into(), "a".into(), 1.0)));
}

#[tokio::test]
async fn one_bad_row_writes_nothing() {
    let p = db("batch");
    assert!(save(
        &p,
        &call(true, &["a"]),
        json!([["a3", "a", 1.0], ["b1", "a", 9.0]])
    )
    .await
    .is_err());
    assert_eq!(
        rows_of(&p).len(),
        2,
        "the in-scope row rolled back with the batch"
    );
}

#[tokio::test]
async fn a_delete_never_reaches_another_entity() {
    let p = db("delete");
    let key = ["id".to_string()];
    let scoped = call(true, &["a"]);
    assert_eq!(
        delete(("sqlite", &p, "setting"), &key, &[json!(["b1"])], &scoped).await,
        Ok(0)
    );
    assert_eq!(
        delete(("sqlite", &p, "setting"), &key, &[json!(["a1"])], &scoped).await,
        Ok(1)
    );
    assert_eq!(rows_of(&p), [("b1".into(), "b".into(), 2.0)]);
}

#[tokio::test]
async fn fails_closed() {
    let p = db("closed");
    assert!(
        save(&p, &call(false, &["a"]), json!([["a1", "a", 5.0]]))
            .await
            .is_err(),
        "not writable"
    );
    assert!(
        save(&p, &call(true, &[]), json!([["a1", "a", 5.0]]))
            .await
            .is_err(),
        "empty set"
    );
    assert_eq!(rows_of(&p)[0].2, 1.0);
}

#[tokio::test]
async fn a_shared_table_is_never_written_by_a_restricted_caller() {
    let p = db("shared");
    let mut shared = call(true, &["a"]);
    shared["write_scope"]["policy"]["tables"]["setting"] = json!({ "kind": "shared" });
    assert!(save(&p, &shared, json!([["a1", "a", 5.0]])).await.is_err());
    let key = ["id".to_string()];
    assert!(
        delete(("sqlite", &p, "setting"), &key, &[json!(["b1"])], &shared)
            .await
            .is_err()
    );
    assert_eq!(rows_of(&p).len(), 2);
}

#[tokio::test]
async fn an_unrestricted_call_writes_as_before() {
    let p = db("open");
    assert_eq!(save(&p, &json!({}), json!([["b1", "a", 7.0]])).await, Ok(1));
    assert!(rows_of(&p).contains(&("b1".into(), "a".into(), 7.0)));
}
