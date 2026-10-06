//! `federation.migrate` against a REAL Postgres, run the way an extension would run it at every
//! start-up: apply the same design twice. The second run must plan nothing and apply cleanly.
//!
//! The existing migrate tests cover SQLite only. This pins the Postgres path: a composite primary
//! key, every neutral column type an extension's tables use, a `DEFAULT now()`, and a foreign key.
//!
//! Requires the `postgres` feature AND a reachable Postgres (`LB_TEST_PG_DSN`, else the repo's dev
//! container on :5433). Without one each test prints a SKIP line and returns green.

#![cfg(feature = "postgres")]

#[allow(dead_code)]
#[path = "../src/event.rs"]
mod event;
#[allow(dead_code)]
#[path = "../src/info_schema.rs"]
mod info_schema;
#[allow(dead_code)]
#[path = "../src/migrate.rs"]
mod migrate;
#[allow(dead_code)]
#[path = "../src/pool.rs"]
mod pool;
#[allow(dead_code)]
#[path = "../src/query.rs"]
mod query;
#[allow(dead_code)]
#[path = "../src/results.rs"]
mod results;
#[allow(dead_code)]
#[path = "../src/source/mod.rs"]
mod source;
#[allow(dead_code)]
#[path = "../src/validate.rs"]
mod validate;

use serde_json::{json, Value};
use source::dialect::DesignSchema;

fn test_dsn() -> String {
    std::env::var("LB_TEST_PG_DSN")
        .unwrap_or_else(|_| "host=localhost port=5433 user=lb password=lb_secret dbname=lb".into())
}

async fn reachable() -> bool {
    match source::connect("postgres", &test_dsn()).await {
        Ok(s) => match s.probe().await {
            Ok(()) => true,
            Err(e) => {
                eprintln!("SKIP: Postgres probe failed ({e})");
                false
            }
        },
        Err(e) => {
            eprintln!("SKIP: Postgres connect failed ({e}); set LB_TEST_PG_DSN");
            false
        }
    }
}

/// Two tables shaped like an extension's own: a lookup table and an entry table that references it.
fn design(prefix: &str) -> DesignSchema {
    serde_json::from_value(json!({
        "tables": [
            {
                "name": format!("{prefix}_stream"),
                "pk": ["id"],
                "columns": [
                    {"name": "id", "type": "text"},
                    {"name": "name", "type": "text"},
                    {"name": "density", "type": "real", "nullable": true},
                    {"name": "sort_order", "type": "integer"}
                ]
            },
            {
                "name": format!("{prefix}_entry"),
                "pk": ["site_ref", "period", "stream_id"],
                "columns": [
                    {"name": "site_ref", "type": "text"},
                    {"name": "period", "type": "text"},
                    {"name": "stream_id", "type": "text"},
                    {"name": "kg", "type": "real", "nullable": true},
                    {"name": "approved", "type": "boolean", "nullable": true},
                    {"name": "entered_at", "type": "timestamp", "default": "now()"}
                ]
            }
        ],
        "fks": [
            {
                "name": format!("{prefix}_entry_stream_fk"),
                "from_table": format!("{prefix}_entry"),
                "from_columns": ["stream_id"],
                "to_table": format!("{prefix}_stream"),
                "to_columns": ["id"]
            }
        ]
    }))
    .unwrap()
}

fn statements(out: &Value) -> Vec<String> {
    out["statements"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| s["sql"].as_str().unwrap_or_default().to_string())
                .collect()
        })
        .unwrap_or_default()
}

#[tokio::test]
async fn applying_the_same_design_twice_changes_nothing_the_second_time() {
    if !reachable().await {
        return;
    }
    let prefix = format!("mig{}", std::process::id());
    let dsn = test_dsn();
    let schema = design(&prefix);

    let first = migrate::run_migrate("postgres", &dsn, &schema, false).await;
    let first = first.unwrap_or_else(|e| panic!("first migrate failed: {e}"));
    assert_eq!(first["applied"], json!(true), "{first}");
    assert!(
        statements(&first)
            .iter()
            .any(|s| s.contains("CREATE TABLE")),
        "the first run creates the tables: {first}"
    );

    // The start-up case: the SAME design again, against the tables it just created.
    let second = migrate::run_migrate("postgres", &dsn, &schema, false).await;
    let second = second.unwrap_or_else(|e| panic!("second migrate failed: {e}"));
    assert!(
        statements(&second).is_empty(),
        "a second run must plan nothing, got: {second}"
    );
}
