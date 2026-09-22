//! The row-policy attack suite (entity-scoped-data scope, test plan). Every escape a hand-written
//! query could try must be refused or filtered; every shape the boards use must still work.

use serde_json::json;

use super::{from_input, rewrite, RowScope};

fn scope(ids: &[&str]) -> RowScope {
    let raw = json!({
        "policy": {
            "tables": {
                "points": {"kind": "keyed", "columns": ["host_uuid", "uuid"]},
                "point_meta_tags": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]},
                "meter_daily_usage_clean": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]},
                "sites": {"kind": "entity", "column": "site"}
            },
            "entity_key": {
                "table": "point_meta_tags", "columns": ["host_uuid", "point_uuid"],
                "key_col": "key", "key_value": "siteRef", "value_col": "value"
            }
        },
        "ids": ids
    });
    from_input(&json!({ "row_scope": raw })).unwrap().unwrap()
}

fn ok(sql: &str) -> String {
    rewrite(sql, &scope(&["Lot 1", "Lot 2"])).unwrap_or_else(|e| panic!("{sql}: {e}"))
}

fn refused(sql: &str) {
    assert!(
        rewrite(sql, &scope(&["Lot 1"])).is_err(),
        "should be refused: {sql}"
    );
}

#[test]
fn a_policy_table_becomes_a_filtered_derived_table() {
    let out = ok("SELECT count(*) FROM points");
    assert!(
        out.contains("FROM (SELECT * FROM points WHERE (host_uuid, uuid) IN"),
        "{out}"
    );
    assert!(
        out.contains("key = 'siteRef' AND value IN ('Lot 1', 'Lot 2')"),
        "{out}"
    );
    assert!(out.contains(") AS points"), "{out}");
}

#[test]
fn aliases_and_qualified_columns_keep_resolving() {
    assert!(ok("SELECT p.name FROM points p").contains(") p"));
    assert!(ok("SELECT points.name FROM points").contains(") AS points"));
    assert!(ok("SELECT * FROM public.points").contains("FROM (SELECT * FROM points WHERE"));
}

#[test]
fn an_entity_table_filters_on_its_own_column() {
    let out = ok("SELECT * FROM sites");
    assert!(out.contains("WHERE site IN ('Lot 1', 'Lot 2')"), "{out}");
}

#[test]
fn a_board_shaped_join_is_rewritten_on_both_sides() {
    let out = ok(
        "WITH m AS (SELECT DISTINCT host_uuid, point_uuid FROM point_meta_tags WHERE key = 'siteRef') \
         SELECT sum(u.step) FROM meter_daily_usage_clean u JOIN m USING (host_uuid, point_uuid) \
         WHERE u.day >= to_timestamp(0)",
    );
    assert_eq!(
        out.matches("WHERE (host_uuid, point_uuid) IN").count(),
        2,
        "{out}"
    );
}

#[test]
fn tables_outside_the_policy_are_refused() {
    refused("SELECT * FROM hosts");
    refused("SELECT * FROM secret.points");
    refused("SELECT * FROM information_schema.tables");
    refused("SELECT * FROM pg_catalog.pg_class");
    refused("SELECT * FROM points p JOIN hosts h ON true");
    refused("SELECT 1 WHERE EXISTS (SELECT 1 FROM hosts)");
    refused("SELECT (SELECT max(x) FROM hosts)");
    refused("SELECT * FROM points, LATERAL (SELECT * FROM hosts) h");
    refused("SELECT name FROM points UNION SELECT name FROM hosts");
    refused("TABLE points");
}

#[test]
fn quoting_follows_postgres_folding() {
    assert!(ok("SELECT * FROM \"points\"").contains("FROM (SELECT * FROM points WHERE"));
    refused("SELECT * FROM \"POINTS\"");
}

#[test]
fn ctes_are_scoped_and_cannot_shadow_a_policy_table() {
    let out = ok("WITH x AS (SELECT * FROM points) SELECT * FROM x");
    assert!(out.contains("FROM (SELECT * FROM points WHERE"), "{out}");
    assert!(out.ends_with("SELECT * FROM x"), "{out}");
    refused("WITH points AS (SELECT 1) SELECT * FROM points");
    refused("WITH x AS (SELECT * FROM hosts) SELECT * FROM x");
    // A CTE defined in a subquery does not hide a real table of the same name outside it.
    refused("SELECT * FROM (WITH hosts AS (SELECT 1) SELECT * FROM hosts) a, hosts");
    ok("WITH RECURSIVE r(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM r WHERE n < 3) SELECT * FROM r");
}

#[test]
fn functions_are_deny_by_default() {
    ok("SELECT date_trunc('day', now()), round(avg(1.0), 2), coalesce(null, 1)");
    ok("SELECT * FROM generate_series(1, 3)");
    refused("SELECT query_to_xml('select * from hosts', true, true, '')");
    refused("SELECT set_config('role', 'postgres', false)");
    refused("SELECT current_setting('is_superuser')");
    refused("SELECT pg_read_file('/etc/passwd')");
    refused("SELECT pg_catalog.now()");
    refused("SELECT * FROM dblink('host=x', 'select 1') AS t(a int)");
    refused("SELECT * FROM query_to_xml('select 1', true, true, '')");
    refused("SELECT count(*) FROM points WHERE name = pg_sleep(10)::text");
}

#[test]
fn ids_are_values_never_sql() {
    let out = rewrite(
        "SELECT * FROM sites",
        &scope(&["O'Brien'); DROP TABLE x; --"]),
    )
    .unwrap();
    assert!(out.contains("'O''Brien''); DROP TABLE x; --'"), "{out}");
}

#[test]
fn no_ids_means_no_rows() {
    let out = rewrite("SELECT * FROM points", &scope(&[])).unwrap();
    assert!(
        out.contains("FROM (SELECT * FROM points WHERE false) AS points"),
        "{out}"
    );
}

#[test]
fn a_bad_policy_is_refused_not_ignored() {
    assert!(from_input(&json!({})).unwrap().is_none());
    let bad_ident = json!({"row_scope": {"ids": [], "policy": {
        "tables": {"points; drop": {"kind": "entity", "column": "site"}},
        "entity_key": {"table": "t", "columns": ["a"], "key_col": "k", "key_value": "v", "value_col": "c"}}}});
    assert!(from_input(&bad_ident).is_err());
    let mismatched = json!({"row_scope": {"ids": [], "policy": {
        "tables": {"points": {"kind": "keyed", "columns": ["a", "b"]}},
        "entity_key": {"table": "t", "columns": ["a"], "key_col": "k", "key_value": "v", "value_col": "c"}}}});
    assert!(from_input(&mismatched).is_err());
    assert!(from_input(&json!({"row_scope": {"ids": "all"}})).is_err());
}

/// Dry run over a real workspace's board SQL (entity-scoped-data rollout step 2). Ignored by
/// default; run with `ROW_POLICY_DRY_RUN=<json [{board,cell,sql}]>` and
/// `cargo test -p federation --bin federation dry_run -- --ignored --nocapture`.
#[test]
#[ignore]
fn dry_run_board_sql() {
    let Ok(path) = std::env::var("ROW_POLICY_DRY_RUN") else {
        return;
    };
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let policy_path = std::env::var("ROW_POLICY_POLICY").ok();
    let scope: RowScope = match policy_path {
        Some(p) => serde_json::from_value(json!({
            "policy": serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(p).unwrap()).unwrap(),
            "ids": ["Lot 1 Sargents Estate - Eastern Creek"]
        }))
        .unwrap(),
        None => scope(&["Lot 1"]),
    };
    let var = regex_lite_vars();
    let mut fails: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let mut passed = 0;
    for r in &rows {
        let raw = r["sql"].as_str().unwrap_or("");
        let sql = var(raw);
        let window =
            json!({"from_ms": 1788000000000_i64, "to_ms": 1790000000000_i64, "width_ms": 3600000});
        let sql = match crate::sql_macros::expand(&sql, "postgres", Some(&window)) {
            Ok(s) => s,
            Err(e) => {
                fails
                    .entry(format!("macro: {e}"))
                    .or_default()
                    .push(format!("{}/{}", r["board"], r["cell"]));
                continue;
            }
        };
        match rewrite(&sql, &scope) {
            Ok(out) => {
                passed += 1;
                if std::env::var("ROW_POLICY_PRINT").ok().as_deref()
                    == Some(&format!(
                        "{}/{}",
                        r["board"].as_str().unwrap_or(""),
                        r["cell"].as_str().unwrap_or("")
                    ))
                {
                    println!("REWRITTEN<<{out}>>");
                }
            }
            Err(e) => fails
                .entry(e.to_string())
                .or_default()
                .push(format!("{}/{}", r["board"], r["cell"])),
        }
    }
    println!("passed {passed} of {}", rows.len());
    for (msg, at) in &fails {
        println!("{:>4}  {msg}   e.g. {}", at.len(), at[0]);
    }
}

/// Fill dashboard variables the way the browser would, closely enough to parse: time built-ins get
/// epoch-ms numbers; `$name` / `${name}` / `${name:fmt}` inside quotes become a word, outside quotes
/// a string literal.
fn regex_lite_vars() -> impl Fn(&str) -> String {
    |sql: &str| {
        let mut out = String::with_capacity(sql.len());
        let b = sql.as_bytes();
        let mut i = 0;
        let mut in_quote = false;
        while i < b.len() {
            let c = b[i] as char;
            if !in_quote && c == '-' && i + 1 < b.len() && b[i + 1] == b'-' {
                let end = sql[i..].find('\n').map(|p| i + p).unwrap_or(b.len());
                out.push_str(&sql[i..end]);
                i = end;
                continue;
            }
            if c == '\'' {
                in_quote = !in_quote;
                out.push(c);
                i += 1;
                continue;
            }
            if c == '$'
                && i + 1 < b.len()
                && (b[i + 1] == b'{'
                    || (b[i + 1] as char).is_ascii_alphabetic()
                    || b[i + 1] == b'_')
            {
                let (name, end) = if b[i + 1] == b'{' {
                    let close = sql[i..].find('}').map(|p| i + p).unwrap_or(b.len() - 1);
                    (
                        sql[i + 2..close]
                            .split(':')
                            .next()
                            .unwrap_or("")
                            .to_string(),
                        close + 1,
                    )
                } else {
                    let mut j = i + 1;
                    while j < b.len() && ((b[j] as char).is_ascii_alphanumeric() || b[j] == b'_') {
                        j += 1;
                    }
                    (sql[i + 1..j].to_string(), j)
                };
                // `$__timeTable(...)` and other `$__` function macros belong to the macro expander.
                if name.starts_with("__")
                    && !matches!(
                        name.as_str(),
                        "__from" | "__to" | "__from_prev" | "__to_prev"
                    )
                {
                    out.push_str(&sql[i..end]);
                    i = end;
                    continue;
                }
                let v = match name.as_str() {
                    "__from" | "__from_prev" => "1788000000000".to_string(),
                    "__to" | "__to_prev" => "1790000000000".to_string(),
                    _ if in_quote => "All".to_string(),
                    // A table-name variable (`FROM ${aggInterval}`) is filled with a table name.
                    _ if out.trim_end().to_ascii_uppercase().ends_with("FROM")
                        || out.trim_end().to_ascii_uppercase().ends_with("JOIN") =>
                    {
                        "daily_data".to_string()
                    }
                    _ => "'All'".to_string(),
                };
                out.push_str(&v);
                i = end;
                continue;
            }
            out.push(c);
            i += 1;
        }
        out
    }
}
