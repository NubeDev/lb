//! The SCHEMAFULL table definitions for the two preference records (prefs scope: "SCHEMAFULL, each
//! axis NULLABLE so unset → inherit is structural"). Composite record ids: `user_prefs:[ws,user]`
//! and `workspace_prefs:[ws]` — deterministic, so an offline edit upserts idempotently on replay
//! (LWW). Namespace-scoped like every table (the workspace is the SurrealDB namespace; the id's
//! `ws` element is denormalized for query convenience, the hard wall is the namespace).
//!
//! Fields are declared `FLEXIBLE TYPE option<...>` so a nullable axis is a first-class absence, and
//! the axis values are validated by serde on the way in (the closed enums), not by SurrealDB asserts
//! — keeping the schema in lock-step with the Rust enums in one place (the enums).

use lb_store::{Store, StoreError};

/// The axis columns to project on a read — explicitly NOT `id` (a composite RecordId whose array
/// id-part does not round-trip cleanly through `serde_json::Value`), only the `Prefs` fields.
pub const PREFS_COLUMNS: &str = "language, timezone, date_style, time_style, first_day_of_week, \
     number_format, unit_system, unit_overrides, ui_theme";

/// The per-(workspace,user) preference table.
pub const USER_PREFS_TABLE: &str = "user_prefs";
/// The per-workspace default preference table.
pub const WORKSPACE_PREFS_TABLE: &str = "workspace_prefs";

/// Define both preference tables in `ws`. Idempotent (`DEFINE ... IF NOT EXISTS`); run on first
/// touch of a workspace. SCHEMAFULL with the axis columns nullable; `unit_overrides` is a flexible
/// object (a closed Dimension→Unit map validated by serde, not the DB).
pub async fn define_prefs_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE TABLE IF NOT EXISTS {USER_PREFS_TABLE} SCHEMAFULL;
         DEFINE FIELD IF NOT EXISTS ws ON {USER_PREFS_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS user ON {USER_PREFS_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS language ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS timezone ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS date_style ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS time_style ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS first_day_of_week ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS number_format ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS unit_system ON {USER_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS unit_overrides ON {USER_PREFS_TABLE} FLEXIBLE TYPE option<object>;
         DEFINE FIELD IF NOT EXISTS ui_theme ON {USER_PREFS_TABLE} FLEXIBLE TYPE option<object>;

         DEFINE TABLE IF NOT EXISTS {WORKSPACE_PREFS_TABLE} SCHEMAFULL;
         DEFINE FIELD IF NOT EXISTS ws ON {WORKSPACE_PREFS_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS language ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS timezone ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS date_style ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS time_style ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS first_day_of_week ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS number_format ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS unit_system ON {WORKSPACE_PREFS_TABLE} TYPE option<string>;
         DEFINE FIELD IF NOT EXISTS unit_overrides ON {WORKSPACE_PREFS_TABLE} FLEXIBLE TYPE option<object>;
         DEFINE FIELD IF NOT EXISTS ui_theme ON {WORKSPACE_PREFS_TABLE} FLEXIBLE TYPE option<object>;"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}
