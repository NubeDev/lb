//! A FLEXIBLE field definition that has lost its `FLEXIBLE` keyword must be REPAIRED at boot.
//!
//! `DEFINE FIELD IF NOT EXISTS` accepts whatever definition it finds, which is fine until the
//! definition it finds is wrong. `FLEXIBLE` is what permits arbitrary keys inside an object on a
//! SCHEMAFULL table; without it SurrealDB rejects every nested key, and it does so only when
//! somebody tries to save — long after boot said everything was fine.
//!
//! This is not hypothetical. Migrating a real SurrealDB 2 store to 3 with `surreal export --v3`
//! re-serializes the schema and drops the keyword: `ui_branding` arrived as plain
//! `TYPE option<object>` and the store then refused its own branding record with
//! "Found field 'ui_branding.faviconDataUri', but no such field exists for table
//! 'workspace_prefs'". Boot could not fix it, because the field existed.
//!
//! So the test stages exactly that damage and asserts the schema pass undoes it.

use lb_prefs::{define_prefs_schema, set_workspace_prefs, Prefs, WORKSPACE_PREFS_TABLE};
use lb_store::Store;
use serde_json::json;

/// The branding blob carries nested keys — which is the whole point of FLEXIBLE.
fn nested_brand() -> serde_json::Value {
    json!({
        "siteName": "ESR",
        "tagline": "building intelligence",
        "faviconDataUri": "data:image/png;base64,iVBORw0KGgo=",
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 1)]
async fn a_flexible_field_stripped_of_flexible_is_repaired_by_the_schema_pass() {
    let store = Store::memory().await.expect("store");

    // Stage the damage a `surreal export --v3` migration leaves behind: the field exists, the
    // table is SCHEMAFULL, and `FLEXIBLE` is gone.
    store
        .query_ws(
            "esr",
            &format!(
                "DEFINE TABLE OVERWRITE {WORKSPACE_PREFS_TABLE} SCHEMAFULL;
                 DEFINE FIELD OVERWRITE ui_branding ON {WORKSPACE_PREFS_TABLE} TYPE option<object>;"
            ),
            vec![],
        )
        .await
        .expect("stage the damaged definition");

    // Boot's schema pass. Under `IF NOT EXISTS` this is a no-op and the write below fails.
    define_prefs_schema(&store, "esr")
        .await
        .expect("schema pass");

    let prefs = Prefs {
        ui_branding: Some(nested_brand()),
        ..Prefs::default()
    };
    set_workspace_prefs(&store, "esr", &prefs, &[])
        .await
        .expect("a nested branding blob must be writable after the schema pass repairs the field");
}
