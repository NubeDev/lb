//! `federation.migrate` engine (schema-designer scope): diff a desired `DesignSchema` (from a
//! `db_schema` record) against a live source's catalog, plan the additive DDL (CREATE TABLE +
//! ALTER ADD COLUMN + ADD CONSTRAINT FK), and optionally apply it. Destructive changes are
//! REFUSED (scope Non-goals: a destructive migration is a named future verb, not a flag here).
//!
//! `dry_run: true` (the default) returns the planned statements without touching the source; the
//! host's "Apply" button is the explicit second step with `dry_run: false` (the Ask gate — a
//! migrate is never silent). When applying, the statements run in ONE transaction where the
//! dialect allows (Postgres DDL is transactional; sqlite wraps the batch in BEGIN/COMMIT) so a
//! half-applied migrate is prevented (scope: "a half-applied migrate is reported per-statement
//! where not [transactional]").
//!
//! Pure DDL generation + diffing lives in `source::dialect` (unit-tested, no IO). This module is
//! the orchestration: connect → read live columns → plan → (apply). The DSN is host-mediated.

use serde_json::{json, Value};

use crate::pool::cached_connect;
use crate::source::dialect::{self, DdlStatement, DesignSchema, DestructiveRefusal, LiveCatalog};

/// Plan + (optionally) apply a migrate of `schema` against the `kind` source at `dsn`. Returns
/// `{statements: [{kind, sql}], applied: bool, destructive_refusal?: string}`. When `dry_run` is
/// true (the default), nothing is applied; when false, the statements run in one transaction.
pub async fn run_migrate(
    kind: &str,
    dsn: &str,
    schema: &DesignSchema,
    dry_run: bool,
) -> Result<Value, String> {
    let source = cached_connect(kind, dsn).await.map_err(|e| e.to_string())?;

    // Read the live catalog: for each desired table, probe its columns directly. `list_columns_
    // with_types` returns an EMPTY vec for a table that does not exist (PRAGMA/information_schema
    // answer empty, never error) — which the diff treats as "brand-new table → CREATE TABLE". This
    // avoids depending on `list_tables()` (a pushed-down catalog scan that mis-resolves on an
    // empty source). Tables the source has but the design omits are NOT touched (dropping is
    // destructive → future verb).
    let mut live = LiveCatalog::default();
    for table in &schema.tables {
        let cols = source
            .list_columns_with_types(&table.name, kind)
            .await
            .map_err(|e| e.to_string())?;
        live.tables.push((table.name.clone(), cols, Vec::new()));
    }

    let plan = match dialect::plan_migrate(schema, &live, kind) {
        Ok(p) => p,
        Err(DestructiveRefusal { message }) => {
            return Ok(json!({
                "statements": [],
                "applied": false,
                "destructive_refusal": message,
            }));
        }
    };

    let statement_json: Vec<Value> = plan
        .statements
        .iter()
        .map(|s| statement_to_json(s))
        .collect();

    if dry_run {
        return Ok(json!({
            "statements": statement_json,
            "applied": false,
        }));
    }

    // Apply in one transaction (the per-kind impl wraps the batch). A failure rolls back — the
    // error names how far the plan got (which statement failed), not "half-applied".
    if plan.statements.is_empty() {
        return Ok(json!({ "statements": [], "applied": false }));
    }
    source
        .apply_ddl(&plan.statements)
        .await
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "statements": statement_json,
        "applied": true,
    }))
}

/// One planned statement → its JSON shape for the UI/agent: `{kind: "create_table"|"add_column"|
/// "add_fk", table, sql, ...}`. The `kind` lets the UI render a DDL preview grouped by action.
fn statement_to_json(s: &DdlStatement) -> Value {
    match s {
        DdlStatement::CreateTable { table, sql } => json!({
            "kind": "create_table", "table": table, "sql": sql
        }),
        DdlStatement::AddColumn { table, column, sql } => json!({
            "kind": "add_column", "table": table, "column": column, "sql": sql
        }),
        DdlStatement::AddFk { table, name, sql } => json!({
            "kind": "add_fk", "table": table, "name": name, "sql": sql
        }),
    }
}

#[cfg(test)]
mod tests {
    //! Migrate-engine orchestration tests run against a REAL seeded SQLite file (no Docker, no
    //! mocks — the source layer is the one sanctioned fake-boundary, testing §0). They pin:
    //!   1. dry_run plans statements but applies nothing;
    //!   2. apply creates the tables live;
    //!   3. re-running migrate on the now-matching schema plans ZERO statements (idempotence);
    //!   4. an additive column change plans exactly one ADD COLUMN;
    //!   5. a destructive change (dropped column) is refused with the what-to-do copy.

    use super::*;
    // The uncached constructor: these tests read the live catalog to VERIFY what apply did, so they
    // must see the real current schema rather than share the pool the code under test warmed.
    use crate::source::connect;
    use crate::source::dialect::{DesignColumn, DesignFk, DesignTable};

    fn unique_db() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("lb-fed-migrate-{seq}-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        path.to_string_lossy().into_owned()
    }

    fn design_shop() -> DesignSchema {
        DesignSchema {
            tables: vec![
                DesignTable {
                    name: "customers".into(),
                    columns: vec![DesignColumn {
                        name: "id".into(),
                        r#type: "integer".into(),
                        nullable: false,
                        default: None,
                    }],
                    pk: vec!["id".into()],
                },
                DesignTable {
                    name: "orders".into(),
                    columns: vec![
                        DesignColumn {
                            name: "id".into(),
                            r#type: "integer".into(),
                            nullable: false,
                            default: None,
                        },
                        DesignColumn {
                            name: "customer_id".into(),
                            r#type: "integer".into(),
                            nullable: false,
                            default: None,
                        },
                    ],
                    pk: vec!["id".into()],
                },
            ],
            fks: vec![DesignFk {
                name: "".into(),
                from_table: "orders".into(),
                from_columns: vec!["customer_id".into()],
                to_table: "customers".into(),
                to_columns: vec!["id".into()],
                on_delete: None,
            }],
        }
    }

    async fn live_table_columns(dsn: &str, table: &str) -> Vec<String> {
        let source = connect("sqlite", dsn).await.unwrap();
        source
            .list_columns_with_types(table, "sqlite")
            .await
            .unwrap()
            .into_iter()
            .map(|c| c.name)
            .collect()
    }

    /// 1 + 2: dry-run plans CREATEs + applies nothing; apply creates the tables live.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dry_run_plans_apply_creates() {
        let dsn = unique_db();
        // empty file: create it so connect() succeeds
        rusqlite::Connection::open(&dsn).unwrap();
        let design = design_shop();

        let dry = run_migrate("sqlite", &dsn, &design, true).await.unwrap();
        assert_eq!(dry["applied"], false, "dry_run does not apply");
        let stmts = dry["statements"].as_array().unwrap();
        assert!(
            stmts.iter().any(|s| s["kind"] == "create_table"),
            "plans CREATE TABLE: {dry}"
        );
        // Nothing applied yet:
        assert!(
            live_table_columns(&dsn, "customers").await.is_empty(),
            "dry_run created nothing"
        );

        let applied = run_migrate("sqlite", &dsn, &design, false).await.unwrap();
        assert_eq!(applied["applied"], true, "apply runs");
        // Live now matches:
        let cols = live_table_columns(&dsn, "orders").await;
        assert!(
            cols.contains(&"customer_id".into()),
            "orders created: {cols:?}"
        );

        let _ = std::fs::remove_file(&dsn);
    }

    /// 3: re-running migrate on a matching schema plans ZERO statements (the idempotence heart).
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn re_run_is_idempotent() {
        let dsn = unique_db();
        rusqlite::Connection::open(&dsn).unwrap();
        let design = design_shop();
        run_migrate("sqlite", &dsn, &design, false).await.unwrap();
        let again = run_migrate("sqlite", &dsn, &design, true).await.unwrap();
        assert_eq!(
            again["statements"].as_array().unwrap().len(),
            0,
            "re-run plans zero statements: {again}"
        );
        let _ = std::fs::remove_file(&dsn);
    }

    /// 4: an additive column change plans exactly one ADD COLUMN after apply.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn additive_column_after_apply() {
        let dsn = unique_db();
        rusqlite::Connection::open(&dsn).unwrap();
        let mut design = design_shop();
        run_migrate("sqlite", &dsn, &design, false).await.unwrap();
        // add a column to the design
        design.tables[1].columns.push(DesignColumn {
            name: "status".into(),
            r#type: "text".into(),
            nullable: true,
            default: None,
        });
        let plan = run_migrate("sqlite", &dsn, &design, true).await.unwrap();
        let stmts = plan["statements"].as_array().unwrap();
        assert_eq!(stmts.len(), 1, "one ADD COLUMN: {plan}");
        assert_eq!(stmts[0]["kind"], "add_column");
        assert_eq!(stmts[0]["column"], "status");
        let _ = std::fs::remove_file(&dsn);
    }

    /// 5: a dropped column is refused with copy that says what to do.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dropped_column_refused() {
        let dsn = unique_db();
        rusqlite::Connection::open(&dsn).unwrap();
        let design = design_shop();
        run_migrate("sqlite", &dsn, &design, false).await.unwrap();
        // destructive: drop a column from the design
        let mut destructive = design.clone();
        destructive.tables[1]
            .columns
            .retain(|c| c.name != "customer_id");
        let plan = run_migrate("sqlite", &dsn, &destructive, true)
            .await
            .unwrap();
        let refusal = plan["destructive_refusal"].as_str().unwrap();
        assert!(
            refusal.contains("additive only"),
            "refusal names the policy: {refusal}"
        );
        assert_eq!(plan["applied"], false);
        assert_eq!(
            plan["statements"].as_array().unwrap().len(),
            0,
            "refused → no statements"
        );
        let _ = std::fs::remove_file(&dsn);
    }
}
