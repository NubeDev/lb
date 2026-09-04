//! What SurrealDB 3 will accept in `ORDER BY` — probed, not guessed.
//!
//! SurrealDB 3 requires the ORDER BY idiom to appear in the statement's selection. The check is in
//! `syn/parser/stmt/parts.rs`: it matches a selected field or its alias EXACTLY, and the
//! prefix relaxation (`idiom_is_prefix`) applies only to `GROUP BY`, never to `ORDER BY`.
//!
//! That breaks lb's generic entity read, which selects the `data` envelope and orders by a path
//! inside it (`SELECT ... data ... ORDER BY data.ts DESC` in `host/src/flows/execute_node/
//! store_crud.rs`). This file records which repair forms the engine actually accepts, so the fix is
//! chosen from evidence rather than assumed.

fn parses(sql: &str) -> Result<(), String> {
    surrealdb_core::syn::parse(sql)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

#[test]
fn ordering_by_a_path_into_a_selected_object_is_rejected() {
    let err = parses("SELECT record::id(id) AS id, data, rev FROM t ORDER BY data.ts DESC")
        .expect_err("SurrealDB 3 must reject ordering by an idiom that is not itself selected");
    assert!(
        err.contains("Missing order idiom"),
        "expected the selection-membership error, got: {err}"
    );
}

#[test]
fn selecting_the_ordered_path_alongside_the_envelope_is_accepted() {
    // Repair A: also select the path. Costs an extra key in every returned row.
    parses("SELECT record::id(id) AS id, data, rev, data.ts FROM t ORDER BY data.ts DESC")
        .expect("selecting the ordered path satisfies the rule");
}

#[test]
fn an_alias_on_the_ordered_path_is_accepted() {
    // Repair B: alias it and order by the alias. Same extra key, but a name we control.
    parses("SELECT record::id(id) AS id, data, rev, data.ts AS _ord FROM t ORDER BY _ord DESC")
        .expect("an alias satisfies the rule");
}

#[test]
fn a_star_in_the_selection_satisfies_the_rule_outright() {
    // Repair C: `*` short-circuits the check ("All is in the idiom so assume the field is
    // present"), but it also returns every column, which is what the explicit list exists to avoid.
    parses("SELECT *, record::id(id) AS id FROM t ORDER BY data.ts DESC")
        .expect("`*` short-circuits the selection check");
}

#[test]
fn the_alias_plus_omit_form_parses() {
    // Repair B is only worth it if OMIT can hide the helper column again.
    parses(
        "SELECT record::id(id) AS id, data, rev, data.ts AS _ord OMIT _ord FROM t ORDER BY _ord DESC",
    )
    .expect("alias + OMIT parses");
}

/// Parsing is not the claim that matters. Run the repair on the REAL engine and check both halves:
/// the rows come back in the right order, AND the helper column is not in the output.
#[tokio::test]
async fn the_alias_plus_omit_form_orders_correctly_and_hides_the_helper() {
    let store = lb_store::Store::memory().await.expect("open");
    for (id, ts) in [("a", 300), ("b", 100), ("c", 200)] {
        store
            .query_ws(
                "ws-a",
                &format!("CREATE ord:{id} SET data = {{ ts: {ts}, name: '{id}' }}, rev = 1"),
                vec![],
            )
            .await
            .expect("seed");
    }

    let mut resp = store
        .query_ws(
            "ws-a",
            "SELECT record::id(id) AS id, data, rev, data.ts AS _ord OMIT _ord \
             FROM ord ORDER BY _ord DESC",
            vec![],
        )
        .await
        .expect("the repaired query must run, not just parse");
    let rows: Vec<serde_json::Value> = resp.take(0).expect("take");

    let order: Vec<&str> = rows.iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(order, vec!["a", "c", "b"], "descending by data.ts");
    assert!(
        rows.iter().all(|r| r.get("_ord").is_none()),
        "OMIT must keep the helper column out of the result: {rows:?}"
    );
}

#[test]
fn ordering_by_a_top_level_selected_field_is_still_fine() {
    // The common case is unaffected — this is a restriction on paths, not on ordering generally.
    parses("SELECT scope, slug, updated_at FROM agent_memory ORDER BY updated_at DESC")
        .expect("ordering by a plainly selected column still parses");
}
