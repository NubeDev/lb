//! `federation.migrate` called by an **extension acting as itself** — the path that records which
//! tables it created ([`super::owned`]) and, on a restricted source, keeps it to those tables.
//!
//! 1. Plan first (the sidecar's dry run): which design tables does this migrate CREATE?
//! 2. On a restricted source, every table the design names — its tables and both ends of every FK —
//!    and every planned statement must be one the plan creates or one the extension already owns.
//!    Anything else (an estate table, another extension's table) refuses the whole migrate.
//! 3. A dry run stops here and returns the plan.
//! 4. Claim the tables the plan creates (atomic, first claim wins), then apply. If the apply fails,
//!    the claims made by this call are released. On an unrestricted source a claim is recorded the
//!    same way, so a policy switched on later keeps the extension working; a lost claim there refuses
//!    nothing, exactly as before.
//!
//! The window between plan and apply: the sidecar re-plans on apply, so a table someone else created
//! in between would be ALTERed rather than created. Claims stop another extension from taking it;
//! a person racing an extension with the same table name inside that window is not covered.

use std::collections::BTreeSet;

use lb_auth::Principal;
use lb_supervisor::Launcher;
use serde_json::Value;

use super::error::FederationError;
use super::migrate::run_sidecar;
use super::owned;
use crate::boot::Node;

/// The migrate for extension `ext`. `input` is the sidecar input without `dry_run`.
// Argument count is the explicit dependency list; bundling it into a struct would be a refactor.
#[allow(clippy::too_many_arguments)]
pub(super) async fn migrate_as_extension<L: Launcher>(
    node: &Node,
    launcher: &L,
    caller: &Principal,
    ws: &str,
    source: &str,
    ext: &str,
    schema: &Value,
    restricted: bool,
    input: &Value,
    dry_run: bool,
    ts: u64,
) -> Result<Value, FederationError> {
    let plan = run_sidecar(node, launcher, caller, ws, &with_dry_run(input, true), ts).await?;
    let creates = created_tables(&plan);

    if restricted {
        let mut named = design_tables(schema);
        named.extend(statement_tables(&plan));
        for table in &named {
            if !creates.contains(table) && !owned::owns(&node.store, ws, ext, source, table).await?
            {
                return Err(FederationError::Denied);
            }
        }
    }
    if dry_run {
        return Ok(plan);
    }

    let mut claimed = Vec::new();
    for table in &creates {
        if owned::owns(&node.store, ws, ext, source, table).await? {
            continue;
        }
        if owned::claim(&node.store, ws, ext, source, table, ts).await? {
            claimed.push(table.clone());
        } else if restricted {
            release_all(node, ws, ext, source, &claimed).await;
            return Err(FederationError::Denied);
        }
    }

    match run_sidecar(node, launcher, caller, ws, &with_dry_run(input, false), ts).await {
        Ok(out) => Ok(out),
        Err(e) => {
            release_all(node, ws, ext, source, &claimed).await;
            Err(e)
        }
    }
}

fn with_dry_run(input: &Value, dry_run: bool) -> Value {
    let mut v = input.clone();
    v["dry_run"] = Value::Bool(dry_run);
    v
}

/// The tables a plan creates (`{statements: [{kind: "create_table", table}]}`).
fn created_tables(plan: &Value) -> BTreeSet<String> {
    statements(plan)
        .filter(|s| s.get("kind").and_then(Value::as_str) == Some("create_table"))
        .filter_map(|s| s.get("table").and_then(Value::as_str).map(str::to_string))
        .collect()
}

/// The table every planned statement touches.
fn statement_tables(plan: &Value) -> BTreeSet<String> {
    statements(plan)
        .filter_map(|s| s.get("table").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn statements(plan: &Value) -> impl Iterator<Item = &Value> {
    plan.get("statements")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// Every table a design names: its tables, and both ends of each FK.
fn design_tables(schema: &Value) -> BTreeSet<String> {
    let names = |list: &str, keys: &[&str]| -> Vec<String> {
        schema
            .get(list)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .flat_map(|item| {
                keys.iter()
                    .filter_map(|k| item.get(*k).and_then(Value::as_str).map(str::to_string))
                    .collect::<Vec<_>>()
            })
            .collect()
    };
    let mut out: BTreeSet<String> = names("tables", &["name"]).into_iter().collect();
    out.extend(names("fks", &["from_table", "to_table"]));
    out
}

async fn release_all(node: &Node, ws: &str, ext: &str, source: &str, tables: &[String]) {
    for table in tables {
        if let Err(e) = owned::release(&node.store, ws, ext, source, table).await {
            tracing::warn!(ws, ext, source, table, error = %e, "federation.migrate: claim not released");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_design_names_its_tables_and_both_ends_of_each_fk() {
        let d = json!({
            "tables": [{"name": "waste_entry"}],
            "fks": [{"from_table": "waste_entry", "to_table": "point_meta_tags"}]
        });
        let got: Vec<String> = design_tables(&d).into_iter().collect();
        assert_eq!(got, ["point_meta_tags", "waste_entry"]);
    }

    #[test]
    fn only_create_table_statements_count_as_created() {
        let plan = json!({"statements": [
            {"kind": "create_table", "table": "waste_entry", "sql": "…"},
            {"kind": "add_column", "table": "point_meta_tags", "column": "x", "sql": "…"}
        ]});
        assert_eq!(
            created_tables(&plan).into_iter().collect::<Vec<_>>(),
            ["waste_entry"]
        );
        assert_eq!(statement_tables(&plan).len(), 2);
    }
}
