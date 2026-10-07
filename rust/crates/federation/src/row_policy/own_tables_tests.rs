//! An extension's own tables under a row policy: read as they are, while every policy table in the
//! same query is still filtered and every other relation is still refused.

use serde_json::json;

use super::{from_input, rewrite, RowScope};

fn scope(own_tables: &[&str]) -> RowScope {
    let raw = json!({
        "policy": {
            "tables": {"point_meta_tags": {"kind": "keyed", "columns": ["host_uuid", "point_uuid"]}},
            "entity_key": {
                "table": "point_meta_tags", "columns": ["host_uuid", "point_uuid"],
                "key_col": "key", "key_value": "siteRef", "value_col": "value"
            }
        },
        "ids": [],
        "own_tables": own_tables
    });
    from_input(&json!({ "row_scope": raw })).unwrap().unwrap()
}

#[test]
fn an_own_table_is_read_as_it_is() {
    let sql = "SELECT site_ref, kg FROM waste_entry WHERE period = '2026-09'";
    let out = rewrite(sql, &scope(&["waste_entry"])).unwrap();
    assert_eq!(out, sql);
}

#[test]
fn a_policy_table_beside_an_own_table_is_still_filtered() {
    let sql = "SELECT w.kg FROM waste_entry w JOIN point_meta_tags t ON t.value = w.site_ref";
    let out = rewrite(sql, &scope(&["waste_entry"])).unwrap();
    assert!(
        out.contains("FROM waste_entry AS w") || out.contains("FROM waste_entry w"),
        "{out}"
    );
    assert!(
        out.contains("(SELECT * FROM point_meta_tags WHERE"),
        "{out}"
    );
}

#[test]
fn a_table_that_is_neither_own_nor_policy_is_still_refused() {
    assert!(rewrite("SELECT * FROM readings", &scope(&["waste_entry"])).is_err());
    assert!(rewrite("SELECT * FROM waste_entry", &scope(&[])).is_err());
}

#[test]
fn an_own_table_must_be_a_plain_identifier() {
    let raw = json!({
        "policy": {
            "tables": {},
            "entity_key": {"table": "t", "columns": ["a"], "key_col": "k", "key_value": "v", "value_col": "x"}
        },
        "ids": [],
        "own_tables": ["Waste\"; DROP"]
    });
    assert!(from_input(&json!({ "row_scope": raw })).is_err());
}
