//! `plan_migrate` when the design is applied again — what an extension does at every start-up.

use super::*;

fn col(name: &str, ty: &str, nullable: bool) -> DesignColumn {
    DesignColumn {
        name: name.into(),
        r#type: ty.into(),
        nullable,
        default: None,
    }
}

fn live_col(name: &str, ty: &str, nullable: bool) -> LiveColumn {
    LiveColumn {
        name: name.into(),
        neutral_type: ty.into(),
        nullable,
    }
}

fn table(name: &str, cols: Vec<DesignColumn>, pk: Vec<&str>) -> DesignTable {
    DesignTable {
        name: name.into(),
        columns: cols,
        pk: pk.iter().map(|s| s.to_string()).collect(),
    }
}

fn live_empty() -> LiveCatalog {
    LiveCatalog::default()
}

/// Re-applying a design whose FK already exists adds nothing — the start-up case. Before, the
/// planner never looked at live constraints and re-emitted every ADD CONSTRAINT, so the second
/// run failed ("constraint already exists") and rolled back the whole migrate.
#[test]
fn an_existing_fk_is_not_added_again() {
    let desired = DesignSchema {
        tables: vec![table(
            "orders",
            vec![col("id", "integer", false), col("user_id", "integer", true)],
            vec!["id"],
        )],
        fks: vec![DesignFk {
            name: "orders_user_fk".into(),
            from_table: "orders".into(),
            from_columns: vec!["user_id".into()],
            to_table: "users".into(),
            to_columns: vec!["id".into()],
            on_delete: None,
        }],
    };
    let live = LiveCatalog {
        tables: vec![(
            "orders".into(),
            vec![
                live_col("id", "integer", false),
                live_col("user_id", "integer", true),
            ],
            vec!["id".into()],
        )],
        fk_names: vec!["orders_user_fk".into()],
    };
    let plan = plan_migrate(&desired, &live, "postgres").unwrap();
    assert!(plan.statements.is_empty(), "{plan:?}");
}

/// Postgres creates each FK ONCE, as its own ADD CONSTRAINT — never also inlined in CREATE
/// TABLE (that produced a second, unnamed `<table>_<col>_fkey` beside the named one).
#[test]
fn a_new_postgres_table_does_not_inline_its_fks() {
    let desired = DesignSchema {
        tables: vec![table(
            "orders",
            vec![col("id", "integer", false), col("user_id", "integer", true)],
            vec!["id"],
        )],
        fks: vec![DesignFk {
            name: "orders_user_fk".into(),
            from_table: "orders".into(),
            from_columns: vec!["user_id".into()],
            to_table: "users".into(),
            to_columns: vec!["id".into()],
            on_delete: None,
        }],
    };
    let plan = plan_migrate(&desired, &live_empty(), "postgres").unwrap();
    let creates: Vec<&str> = plan
        .statements
        .iter()
        .filter(|s| matches!(s, DdlStatement::CreateTable { .. }))
        .map(|s| s.sql())
        .collect();
    assert!(!creates[0].contains("REFERENCES"), "{}", creates[0]);
    let fks = plan
        .statements
        .iter()
        .filter(|s| matches!(s, DdlStatement::AddFk { .. }))
        .count();
    assert_eq!(fks, 1);
}
