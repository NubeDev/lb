//! The live catalog `federation.migrate` diffs against, read straight from `information_schema`.
//!
//! The trait default reads a table provider's Arrow schema, which on Postgres answered EMPTY for a
//! table that exists. The planner reads "no columns" as "no table", so every re-applied design planned
//! the same CREATE TABLE again and failed ("relation already exists") — fatal for an extension that
//! migrates at every start-up. Both reads use `current_schema()`: the schema an unqualified name
//! resolves to, which is where `apply_ddl`'s unqualified CREATE lands and where reads and writes find
//! the table.

use super::{LiveColumn, SourceError};

/// Columns of `table`, typed through `canonicalize_live_type` (Postgres' own `data_type` names —
/// `double precision`, `timestamp without time zone`, … — are what it maps). Empty: no such table.
pub(super) async fn columns(
    client: &tokio_postgres::Client,
    table: &str,
    kind: &str,
) -> Result<Vec<LiveColumn>, SourceError> {
    let rows = client
        .query(
            "SELECT column_name::text, data_type::text, is_nullable::text \
             FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = $1 \
             ORDER BY ordinal_position",
            &[&table],
        )
        .await
        .map_err(|e| SourceError(format!("catalog columns: {e}")))?;
    Ok(rows
        .iter()
        .map(|r| {
            let ty: String = r.get(1);
            let nullable: String = r.get(2);
            LiveColumn {
                name: r.get(0),
                neutral_type: super::dialect::canonicalize_live_type(&ty, kind),
                nullable: nullable == "YES",
            }
        })
        .collect())
}

/// The FOREIGN KEY constraint names already on `table`, so the planner does not add them again.
pub(super) async fn fk_names(
    client: &tokio_postgres::Client,
    table: &str,
) -> Result<Vec<String>, SourceError> {
    let rows = client
        .query(
            "SELECT constraint_name::text FROM information_schema.table_constraints \
             WHERE table_schema = current_schema() AND table_name = $1 \
               AND constraint_type = 'FOREIGN KEY'",
            &[&table],
        )
        .await
        .map_err(|e| SourceError(format!("catalog constraints: {e}")))?;
    Ok(rows.iter().map(|r| r.get(0)).collect())
}
