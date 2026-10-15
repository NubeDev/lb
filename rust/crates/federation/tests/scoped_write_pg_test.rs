//! Entity-scoped writes on a real Postgres (`entity-scoped-writes-scope.md`): the guarded upsert and
//! delete the generator builds (`source::write_sql`), run by the postgres source (`pg_write`). The
//! sqlite twin of every case is `row_policy::write_tests`; this pins the `$n` statement and the
//! rollback on postgres, where the guard's ids bind as text after the row's own cells.
//!
//! Requires the `postgres` feature AND a reachable Postgres (`LB_TEST_PG_DSN`, else the repo's dev
//! container on :5433). Unreachable ⇒ a printed SKIP, never a silent green.

#![cfg(feature = "postgres")]

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

use serde_json::{json, Value};
use source::{Source, WriteGuard};

fn test_dsn() -> String {
    // libpq KEY=VALUE form, as `direct_path_pg_test` explains. Test credential against a local
    // container — never a production secret.
    std::env::var("LB_TEST_PG_DSN").unwrap_or_else(|_| {
        "host=localhost port=5433 user=lb password=lb_secret dbname=lb".to_string()
    })
}

/// A fresh table `t` with one row for site a and one for site b, or `None` (SKIP) without Postgres.
async fn seeded(t: &str) -> Option<std::sync::Arc<dyn Source>> {
    let s = match source::connect("postgres", &test_dsn()).await {
        Ok(s) if s.probe().await.is_ok() => s,
        other => {
            eprintln!(
                "SKIP: no Postgres ({:?}); set LB_TEST_PG_DSN",
                other.err().map(|e| e.to_string())
            );
            return None;
        }
    };
    for sql in [
        format!("DROP TABLE IF EXISTS {t}"),
        format!("CREATE TABLE {t} (id TEXT PRIMARY KEY, site_id TEXT, value DOUBLE PRECISION)"),
        format!("INSERT INTO {t} VALUES ('a1', 'a', 1), ('b1', 'b', 2)"),
    ] {
        s.exec_raw_for_test(&sql).await.expect("seed");
    }
    Some(s)
}

fn guard() -> WriteGuard {
    WriteGuard {
        column: "site_id".into(),
        ids: vec!["a".into()],
    }
}

fn cols() -> Vec<String> {
    ["id", "site_id", "value"].map(String::from).to_vec()
}

async fn rows_of(s: &dyn Source, t: &str) -> Vec<Value> {
    s.query_direct_json(&format!("SELECT id, site_id, value FROM {t} ORDER BY id"))
        .await
        .unwrap()
        .1
}

#[tokio::test]
async fn a_guarded_upsert_writes_own_rows_and_never_takes_over_another() {
    let t = "scoped_upsert";
    let Some(s) = seeded(t).await else { return };
    let key = ["id".to_string()];
    let own = vec![
        vec![json!("a1"), json!("a"), json!(10.0)],
        vec![json!("a2"), json!("a"), json!(3.0)],
    ];
    assert_eq!(
        s.write_rows(t, &cols(), &own, Some(&key), Some(&guard()))
            .await
            .unwrap(),
        2
    );
    // `b1` belongs to site b: claiming it as site a must change nothing, and roll back the batch.
    let takeover = vec![
        vec![json!("a3"), json!("a"), json!(1.0)],
        vec![json!("b1"), json!("a"), json!(99.0)],
    ];
    assert!(s
        .write_rows(t, &cols(), &takeover, Some(&key), Some(&guard()))
        .await
        .is_err());
    assert_eq!(
        rows_of(&*s, t).await,
        [
            json!(["a1", "a", 10.0]),
            json!(["a2", "a", 3.0]),
            json!(["b1", "b", 2.0])
        ]
    );
}

#[tokio::test]
async fn a_guarded_delete_never_reaches_another_site() {
    let t = "scoped_delete";
    let Some(s) = seeded(t).await else { return };
    let key = ["id".to_string()];
    assert_eq!(
        s.delete_rows(t, &key, &[vec![json!("b1")]], Some(&guard()))
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        s.delete_rows(t, &key, &[vec![json!("a1")]], Some(&guard()))
            .await
            .unwrap(),
        1
    );
    assert_eq!(rows_of(&*s, t).await, [json!(["b1", "b", 2.0])]);
}
