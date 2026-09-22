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
