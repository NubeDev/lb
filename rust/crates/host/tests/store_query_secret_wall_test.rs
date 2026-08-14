//! The **raw-read wall over the secret plane** (node-update scope, decision 9), end to end against a
//! real store — no mocks, no fake store, the same `store_query_run` / `store_scan_view` /
//! `store_graph_view` entry points the MCP surfaces call.
//!
//! The hole this closes: `secret.get` has an owner gate, but `mcp:store.query:call` is an
//! **author-tier** cap, so before this wall a plain member could read the sealed value in plaintext
//! with `SELECT * FROM secret`. `store.scan`/`store.graph` had the same property at admin tier.
//!
//! Every case below seeds a REAL secret row through `lb_store::write` (the host-internal write path)
//! first, so a passing test would mean the plaintext actually came back — the refusal is what stops
//! it, not an empty table.

use lb_auth::{mint, verify, Claims, Principal, Role, SigningKey};
use lb_host::{
    store_graph_view, store_query_run, store_scan_view, DbViewError, StoreQueryError,
    MAX_QUERY_ROWS,
};
use lb_store::Store;
use serde_json::json;

fn principal(sub: &str, ws: &str, caps: &[&str]) -> Principal {
    let key = SigningKey::generate();
    let claims = Claims {
        sub: sub.into(),
        ws: ws.into(),
        role: Role::Member,
        caps: caps.iter().map(|s| s.to_string()).collect(),
        iat: 0,
        exp: u64::MAX,
        constraint: None,
        run_id: None,
    };
    let token = mint(&key, &claims);
    verify(&key, &token, 1).expect("token verifies")
}

const QUERY: &str = "mcp:store.query:call";
const SCAN: &str = "mcp:store.scan:call";
const GRAPH: &str = "mcp:store.graph:call";

/// A real store with a real sealed-shaped secret row AND an ordinary row to prove non-over-refusal.
async fn seeded(ws: &str) -> Store {
    let store = Store::memory().await.unwrap();
    lb_store::write(
        &store,
        ws,
        "secret",
        "update-token",
        &json!({ "id": "update-token", "value": "PLAINTEXT-CREDENTIAL", "owner": "node:host" }),
    )
    .await
    .expect("seed secret");
    lb_store::write(
        &store,
        ws,
        "site",
        "hq",
        &json!({ "id": "hq", "name": "HQ" }),
    )
    .await
    .expect("seed ordinary row");
    store
}

fn assert_secret_refusal(err: StoreQueryError, sql: &str) {
    match err {
        StoreQueryError::SecretTable(t) => assert!(
            sql.to_ascii_lowercase().contains(t),
            "the refusal names the secret table the statement touched ({t}): {sql}"
        ),
        // A dynamic table position is refused as `Rejected` (we cannot prove which table it is) —
        // also a refusal, and the message must say so rather than mention a store fault.
        StoreQueryError::Rejected(m) => assert!(
            m.contains("run time") || m.contains("secret"),
            "refusal explains itself for `{sql}`: {m}"
        ),
        other => panic!("`{sql}` must be refused by the secret wall, got {other:?}"),
    }
}

/// The headline hole: a bare `SELECT * FROM secret` by an author-tier caller.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn direct_select_from_secret_is_refused() {
    let ws = "sw-direct";
    let store = seeded(ws).await;
    let p = principal("user:test", ws, &[QUERY]);

    let err = store_query_run(&store, &p, ws, "SELECT * FROM secret", vec![])
        .await
        .expect_err("a direct read of the secret plane is refused");
    assert!(
        matches!(err, StoreQueryError::SecretTable("secret")),
        "typed, table-naming refusal — not a generic 500: {err:?}"
    );
    assert!(
        err.to_string().contains("secret"),
        "the message names the table: {err}"
    );
}

/// The shapes a string blocklist or a table-position-only check would miss: aliases, projections,
/// subqueries, a comma JOIN, a record-id read, a graph part, and `INFO FOR TABLE`.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn every_indirect_route_into_the_secret_plane_is_refused() {
    let ws = "sw-indirect";
    let store = seeded(ws).await;
    let p = principal("user:test", ws, &[QUERY]);

    for sql in [
        // aliased / projected — the secret table only appears in the FROM
        "SELECT data.value AS v FROM secret",
        // a subquery — the outer table is innocent
        "SELECT * FROM (SELECT data.value FROM secret)",
        "SELECT (SELECT data.value FROM secret) AS leaked FROM site",
        // a JOIN-shaped multi-table FROM
        "SELECT * FROM site, secret",
        // a record id, and a range over the table
        "SELECT * FROM secret:`update-token`",
        // a graph traversal into the plane
        "SELECT ->holds->secret.* FROM site",
        // introspection discloses the plane's shape
        "INFO FOR TABLE secret",
        // the other secret-plane tables share the wall
        "SELECT * FROM apikey",
        "SELECT * FROM identity_credential",
        "SELECT * FROM credential",
        // casing is not an escape hatch
        "SELECT * FROM SECRET",
    ] {
        let err = store_query_run(&store, &p, ws, sql, vec![])
            .await
            .unwrap_err_or_panic(sql);
        assert_secret_refusal(err, sql);
    }
}

/// A table chosen at run time is judged by what it RESOLVES to, using the bindings the same request
/// supplies — and refused as unprovable only when nothing resolves it.
///
/// The wall's promise is "no read of the secret plane", not "no parameterised table". `$t` is not
/// unknowable when the caller hands over `vars = {t: …}` in the same call: the table it will read is
/// known here, so it is checked by name. That is strictly stronger than blanket refusal (the
/// dangerous binding is now named in the error), and it is what lets the injection-safe form through
/// — the platform `store-read` node parameterises its table precisely so that user text is never
/// spliced into SQL, and blanket refusal made that node fail every time it ran.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_dynamic_table_is_resolved_from_its_binding_and_refused_when_unprovable() {
    let ws = "sw-dynamic";
    let store = seeded(ws).await;
    let p = principal("user:test", ws, &[QUERY]);

    // The live bypass, still shut — and now the refusal NAMES the table the binding chose.
    for (sql, vars) in [
        (
            "SELECT * FROM type::table($t)",
            vec![("t".to_string(), json!("secret"))],
        ),
        ("SELECT * FROM $t", vec![("t".to_string(), json!("secret"))]),
        // …including through a subquery, where the table position is inherited.
        (
            "SELECT * FROM (SELECT * FROM type::table($t))",
            vec![("t".to_string(), json!("secret"))],
        ),
        // …and the literal form, refused by name as before.
        ("SELECT * FROM type::table('secret')", vec![]),
    ] {
        let err = store_query_run(&store, &p, ws, sql, vars)
            .await
            .unwrap_err_or_panic(sql);
        assert!(
            matches!(err, StoreQueryError::SecretTable("secret")),
            "a binding that chooses the secret plane is refused BY NAME for `{sql}`: {err:?}"
        );
    }

    // The innocent binding resolves to an ordinary table and reads it — the parameterised form the
    // `store-read` node depends on.
    let result = store_query_run(
        &store,
        &p,
        ws,
        "SELECT data.name AS name FROM type::table($t)",
        vec![("t".to_string(), json!("site"))],
    )
    .await
    .expect("a bound table position resolves to an ordinary table");
    assert_eq!(
        result.rows.len(),
        1,
        "the innocent bound form reads for real"
    );

    // The remaining honest false-refusal edge: nothing binds the table, so nothing proves it. An
    // unbound param, a param bound to a non-string, and a computed idiom are all still refused.
    for (sql, vars) in [
        ("SELECT * FROM type::table($t)", vec![]),
        (
            "SELECT * FROM type::table($t)",
            vec![("t".to_string(), json!({ "not": "a table name" }))],
        ),
        ("SELECT * FROM some.field", vec![]),
    ] {
        let err = store_query_run(&store, &p, ws, sql, vars)
            .await
            .unwrap_err_or_panic(sql);
        assert!(
            matches!(err, StoreQueryError::Rejected(ref m) if m.contains("run time")),
            "unresolvable stays refused as unprovable for `{sql}`: {err:?}"
        );
    }
}

/// A **composed** read — the shape every grid/plan builder produces, and the shape `store.query`
/// itself produces when it wraps a validated SELECT for the row cap. The subquery's own projection,
/// `WHERE` and `ORDER BY` name no table, so inheriting the table position past the statement
/// boundary refused all of them (each field reference read as a runtime-computed table).
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_composed_subquery_read_is_not_refused_for_its_own_field_references() {
    let ws = "sw-composed";
    let store = seeded(ws).await;
    let p = principal("user:test", ws, &[QUERY]);

    let result = store_query_run(
        &store,
        &p,
        ws,
        "SELECT * FROM (SELECT data.name AS name FROM site ORDER BY data.name) WHERE name = 'HQ'",
        vec![],
    )
    .await
    .expect("a composed read over an ordinary table is not the wall's business");
    assert_eq!(result.rows.len(), 1);

    // …and the boundary does not open a door: a secret read nested inside the subquery, in the
    // subquery's OWN table position, is still refused.
    for sql in [
        "SELECT * FROM (SELECT * FROM secret)",
        "SELECT * FROM site WHERE name IN (SELECT VALUE value FROM secret)",
        "SELECT * FROM (SELECT * FROM (SELECT * FROM secret))",
    ] {
        let err = store_query_run(&store, &p, ws, sql, vec![])
            .await
            .unwrap_err_or_panic(sql);
        assert_secret_refusal(err, sql);
    }
}

/// No over-refusal: ordinary tables still query, with the same shape as before the wall.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn an_ordinary_table_still_queries_fine() {
    let ws = "sw-ok";
    let store = seeded(ws).await;
    let p = principal("user:test", ws, &[QUERY]);

    let result = store_query_run(&store, &p, ws, "SELECT data.name AS name FROM site", vec![])
        .await
        .expect("an ordinary read is untouched by the wall");
    assert_eq!(result.rows.len(), 1);
    assert_eq!(result.rows[0]["name"], json!("HQ"));
    assert!(result.columns.contains(&"name".to_string()));

    // Functions, params and subqueries are only refused in a TABLE position — an ordinary query that
    // uses them elsewhere must still run.
    let result = store_query_run(
        &store,
        &p,
        ws,
        "SELECT string::lowercase(data.name) AS lower FROM site WHERE data.name = $n",
        vec![("n".into(), json!("HQ"))],
    )
    .await
    .expect("params/functions outside a table position are fine");
    assert_eq!(result.rows.len(), 1, "the WHERE $n param bound");
    assert_eq!(result.rows[0]["lower"], json!("hq"));
    assert!(MAX_QUERY_ROWS > 0);
}

/// `store.scan` — the admin-tier raw grid — is walled too, and still scans ordinary tables.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_scan_refuses_the_secret_plane_but_not_ordinary_tables() {
    let ws = "sw-scan";
    let store = seeded(ws).await;
    let p = principal("user:admin", ws, &[SCAN]);

    for table in ["secret", "SECRET", "apikey", "credential"] {
        let err = store_scan_view(&store, &p, ws, table, 10, None)
            .await
            .expect_err("scan of the secret plane is refused for a workspace admin too");
        assert!(
            matches!(err, DbViewError::SecretTable(_)),
            "typed refusal naming the table: {err:?}"
        );
    }

    let page = store_scan_view(&store, &p, ws, "site", 10, None)
        .await
        .expect("an ordinary scan is untouched");
    assert_eq!(page.rows.len(), 1, "the ordinary row still scans");
}

/// `store.graph` — both seeds (a table, and the table half of a `table:id` record) are walled.
#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn store_graph_refuses_secret_seeds_but_not_ordinary_ones() {
    let ws = "sw-graph";
    let store = seeded(ws).await;
    let p = principal("user:admin", ws, &[GRAPH]);

    let err = store_graph_view(&store, &p, ws, Some("secret"), None, 1)
        .await
        .expect_err("a secret table seed is refused");
    assert!(matches!(err, DbViewError::SecretTable("secret")));

    let err = store_graph_view(&store, &p, ws, None, Some("secret:update-token"), 1)
        .await
        .expect_err("a secret record seed is refused");
    assert!(matches!(err, DbViewError::SecretTable("secret")));

    store_graph_view(&store, &p, ws, Some("site"), None, 1)
        .await
        .expect("an ordinary seed still walks the graph");
}

/// A tiny helper so a loop over statements reports WHICH statement leaked instead of a bare unwrap.
trait ExpectRefused<T, E> {
    fn unwrap_err_or_panic(self, sql: &str) -> E;
}
impl<T: std::fmt::Debug, E> ExpectRefused<T, E> for Result<T, E> {
    fn unwrap_err_or_panic(self, sql: &str) -> E {
        match self {
            Ok(ok) => panic!("`{sql}` MUST be refused by the secret wall, but it ran: {ok:?}"),
            Err(e) => e,
        }
    }
}
