//! The SCHEMAFULL `message_catalog` table (i18n-catalogs scope, pinned schema). One sparse override
//! record per `(ws, locale)` — a composite record id `message_catalog:[ws, locale]`, deterministic
//! so an offline override edit upserts idempotently on replay (LWW per message-key). Namespace-
//! scoped like every table (the workspace is the SurrealDB namespace; the id's `ws` element is
//! denormalized for query convenience, the hard wall is the namespace).
//!
//! `messages` is a FLEXIBLE object — a flat `{ "alert.threshold_crossed": "<MF1>" }` map, keys are
//! flat dotted strings (never nested). The MF1 subset of a message is validated by the host's
//! catalog-lint on write, not by SurrealDB (keeping the grammar in one place — the parser).

use lb_store::{Store, StoreError};

/// The per-(workspace,locale) override-catalog table.
pub const CATALOG_TABLE: &str = "message_catalog";

/// The columns to project on a read — explicitly NOT `id` (a composite RecordId whose array id-part
/// does not round-trip cleanly through `serde_json::Value`), only the data fields.
pub const CATALOG_COLUMNS: &str = "ws, locale, messages";

/// Define the `message_catalog` table in `ws`. Idempotent (`DEFINE ... IF NOT EXISTS`). SCHEMAFULL
/// with `messages` a flexible object (the flat key→MF1 map).
pub async fn define_catalog_schema(store: &Store, ws: &str) -> Result<(), StoreError> {
    let sql = format!(
        "DEFINE TABLE IF NOT EXISTS {CATALOG_TABLE} SCHEMAFULL;
         DEFINE FIELD IF NOT EXISTS ws ON {CATALOG_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS locale ON {CATALOG_TABLE} TYPE string;
         DEFINE FIELD IF NOT EXISTS messages ON {CATALOG_TABLE} FLEXIBLE TYPE option<object>;"
    );
    store.query_ws(ws, &sql, vec![]).await?;
    Ok(())
}
