//! The `db_schema:{ws}:{name}` store record (schema-designer scope). The **designed** schema
//! document: tables, columns (with dialect-neutral types), PKs, FKs, and canvas layout geometry.
//! This is THE product — the canvas is one editor of it; `dbschema.save` is callable by the agent
//! just as well as by the UI (MCP is the universal contract, rule 7). The record is
//! workspace-keyed (rule 6 — `db_schema:{ws}:{name}`); `dbschema.*` resolve only in the caller's
//! workspace (ws-B cannot name a ws-A schema).
//!
//! Types are stored dialect-NEUTRAL (`text`/`integer`/`real`/…), mapped per-kind at migrate plan
//! time (open-question lean #2: one schema, many targets). The `v: 1` version field rides the
//! record from day one so a future additive shape (indexes, checks, enums) is a clean up-convert,
//! not a breaking change (scope Risk 6).

use lb_store::{list as store_list, read, write, Store, StoreError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The current `db_schema` record shape version. A future reader compares to up-convert.
pub const SCHEMA_VERSION: u32 = 1;

/// The store table for designed-schema records (one place owns the name).
pub const TABLE: &str = "db_schema";

/// A designed schema record — the workspace-keyed product a canvas edits or an agent authors via
/// `dbschema.save`. `tables` + `fks` + `layout` is the full v1 surface; everything else (indexes,
/// checks, enums, partitions) is an explicit follow-up on a versioned shape (scope Risk 6).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DbSchemaRecord {
    pub name: String,
    /// The record shape version. Today always 1; compared on read to up-convert future shapes.
    pub version: u32,
    pub tables: Vec<DesignTable>,
    #[serde(default)]
    pub fks: Vec<DesignFk>,
    /// Canvas geometry: table name → {x, y}. Rides the record (the Node.position precedent from
    /// flows) so the picture survives reload. The server treats this as opaque layout data.
    #[serde(default)]
    pub layout: BTreeMap<String, LayoutPos>,
    /// The constant discriminator so `dbschema.list` enumerates via the store's field-equality list.
    #[serde(default = "schema_tag")]
    pub tag: String,
    /// A soft-delete marker (`dbschema.delete`): a removed schema reads as absent (the store has no
    /// delete verb; a tombstone keeps the id stable + idempotent, mirroring `datasource.remove`).
    #[serde(default)]
    pub removed: bool,
    /// Caller-injected logical timestamp (no wall-clock — testing §3). Set by the host verb from
    /// the call's `ts` arg (the record's own value is ignored on save).
    #[serde(default)]
    pub ts: u64,
}

/// One designed table. Columns are dialect-neutral; `pk` names the primary-key column set.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignTable {
    pub name: String,
    pub columns: Vec<DesignColumn>,
    #[serde(default)]
    pub pk: Vec<String>,
}

/// One designed column. `r#type` is a canonical neutral type, validated at save.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default)]
    pub nullable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// One designed foreign key: `from_table.from_columns` → `to_table.to_columns`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignFk {
    #[serde(default)]
    pub name: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_delete: Option<String>,
}

/// Canvas geometry for one table node (opaque to the server — the canvas owns the meaning).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayoutPos {
    pub x: f64,
    pub y: f64,
}

/// The constant `tag` value every `db_schema` record carries (the list discriminator).
pub fn schema_tag() -> String {
    "dbschema".to_string()
}

impl DbSchemaRecord {
    /// Build a fresh v1 record. `tag` is stamped automatically.
    pub fn new(name: impl Into<String>, ts: u64) -> Self {
        Self {
            name: name.into(),
            version: SCHEMA_VERSION,
            tables: Vec::new(),
            fks: Vec::new(),
            layout: BTreeMap::new(),
            tag: schema_tag(),
            removed: false,
            ts,
        }
    }
}

/// Persist (upsert) a `db_schema` record in `ws`. Workspace-namespaced by the store (the hard wall).
pub async fn put(store: &Store, ws: &str, rec: &DbSchemaRecord) -> Result<(), StoreError> {
    let value = serde_json::to_value(rec).map_err(|e| StoreError::Decode(e.to_string()))?;
    write(store, ws, TABLE, &rec.name, &value).await
}

/// Resolve `name` to its `db_schema` record in `ws`. `None` if absent OR tombstoned — which is
/// exactly what a cross-tenant name resolves to (a ws-B caller naming a ws-A schema finds nothing,
/// the workspace wall made structural at the namespace).
pub async fn resolve(
    store: &Store,
    ws: &str,
    name: &str,
) -> Result<Option<DbSchemaRecord>, StoreError> {
    let Some(value) = read(store, ws, TABLE, name).await? else {
        return Ok(None);
    };
    let rec: DbSchemaRecord = decode(value)?;
    if rec.removed {
        return Ok(None);
    }
    Ok(Some(rec))
}

/// List the (non-removed) `db_schema` records in `ws` — name + table count (no layout/geometry in
/// the summary; the list is for browsing, the full record loads on open).
pub async fn list_summaries(store: &Store, ws: &str) -> Result<Vec<DbSchemaSummary>, StoreError> {
    let rows = store_list(store, ws, TABLE, "tag", &schema_tag()).await?;
    let mut out = Vec::new();
    for value in rows {
        let rec: DbSchemaRecord = decode(value)?;
        if rec.removed {
            continue;
        }
        out.push(DbSchemaSummary {
            name: rec.name,
            table_count: rec.tables.len() as u32,
            version: rec.version,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

/// A list row for `dbschema.list` — name + table count, deliberately no layout (a browse row).
#[derive(Debug, Clone, Serialize)]
pub struct DbSchemaSummary {
    pub name: String,
    pub table_count: u32,
    pub version: u32,
}

fn decode(value: Value) -> Result<DbSchemaRecord, StoreError> {
    let mut rec: DbSchemaRecord =
        serde_json::from_value(value).map_err(|e| StoreError::Decode(e.to_string()))?;
    // Up-convert: a future reader compares `version` to SCHEMA_VERSION. Today every record is v1,
    // so this is a no-op; the field rides from day one (scope Risk 6).
    if rec.version == 0 {
        rec.version = 1;
    }
    Ok(rec)
}

/// Validate a record's structure before save (defense in depth — the UI validates too). Returns
/// `Ok(())` if every table/column/PK name is a safe identifier and every column type is in the
/// canonical neutral vocabulary. This is what stops a `dbschema.save` from persisting a name that
/// would break the DDL generator's quoting (the generator double-quotes + the identifier check in
/// `federation.write` re-checks, but catching it at save gives the author a clear error).
pub fn validate_record(rec: &DbSchemaRecord) -> Result<(), String> {
    let neutral = neutral_type_set();
    let mut table_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for t in &rec.tables {
        validate_identifier(&t.name, "table name")?;
        if !table_names.insert(t.name.as_str()) {
            return Err(format!("duplicate table name `{}`", t.name));
        }
        let mut col_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for c in &t.columns {
            validate_identifier(&c.name, &format!("column name in `{}`", t.name))?;
            if !col_names.insert(c.name.as_str()) {
                return Err(format!(
                    "duplicate column `{}` in table `{}`",
                    c.name, t.name
                ));
            }
            if !neutral.contains(&c.r#type.as_str()) {
                return Err(format!(
                    "column `{}.{}` has unknown type `{}` (allowed: {})",
                    t.name,
                    c.name,
                    c.r#type,
                    neutral.join(", ")
                ));
            }
        }
        for pk in &t.pk {
            if !col_names.contains(pk.as_str()) {
                return Err(format!(
                    "primary-key column `{}` is not a column of table `{}`",
                    pk, t.name
                ));
            }
        }
    }
    for fk in &rec.fks {
        if !fk.name.is_empty() {
            validate_identifier(&fk.name, "fk name")?;
        }
        if !table_names.contains(fk.from_table.as_str()) {
            return Err(format!(
                "fk from_table `{}` is not a designed table",
                fk.from_table
            ));
        }
        if !table_names.contains(fk.to_table.as_str()) {
            return Err(format!(
                "fk to_table `{}` is not a designed table",
                fk.to_table
            ));
        }
        if fk.from_columns.is_empty() || fk.to_columns.is_empty() {
            return Err(format!(
                "fk `{}` must name at least one from-column and one to-column",
                fk.name
            ));
        }
    }
    Ok(())
}

/// The canonical neutral type vocabulary (mirrors the sidecar's `dialect::NEUTRAL_TYPES`). Kept in
/// the host crate so `dbschema.save` validates WITHOUT importing the sidecar (the host never links
/// the federation crate — it speaks the sidecar wire).
fn neutral_type_set() -> &'static [&'static str] {
    &[
        "text",
        "integer",
        "real",
        "boolean",
        "blob",
        "date",
        "timestamp",
        "numeric",
        "json",
    ]
}

/// `[a-zA-Z_][a-zA-Z0-9_]*` — the same identifier rule the sidecar's DDL generator + write path
/// enforce. Centralized here so the save-time validation and the apply-time check agree.
fn validate_identifier(name: &str, ctx: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err(format!("empty {ctx}"));
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !(first.is_ascii_alphabetic() || first == '_') {
        return Err(format!(
            "`{name}` ({ctx}) must start with a letter or underscore"
        ));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(format!(
            "`{name}` ({ctx}) may contain only letters, digits, or underscore"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn col(name: &str, ty: &str) -> DesignColumn {
        DesignColumn {
            name: name.into(),
            r#type: ty.into(),
            nullable: true,
            default: None,
        }
    }

    fn table(name: &str, cols: Vec<DesignColumn>) -> DesignTable {
        DesignTable {
            name: name.into(),
            columns: cols,
            pk: vec![],
        }
    }

    #[test]
    fn valid_record_passes() {
        let mut rec = DbSchemaRecord::new("shop", 1);
        rec.tables.push(table("users", vec![col("id", "integer")]));
        rec.tables.push(table(
            "orders",
            vec![col("id", "integer"), col("amount", "real")],
        ));
        rec.tables[0].pk = vec!["id".into()];
        rec.tables[1].pk = vec!["id".into()];
        rec.fks.push(DesignFk {
            name: "".into(),
            from_table: "orders".into(),
            from_columns: vec!["user_id".into()],
            to_table: "users".into(),
            to_columns: vec!["id".into()],
            on_delete: None,
        });
        assert!(validate_record(&rec).is_ok(), "valid record");
    }

    #[test]
    fn rejects_bad_identifier() {
        let mut rec = DbSchemaRecord::new("shop", 1);
        rec.tables.push(table("ev\"il", vec![col("id", "integer")]));
        let err = validate_record(&rec).unwrap_err();
        assert!(
            err.contains("must start with a letter") || err.contains("may contain only"),
            "should flag the bad identifier: {err}"
        );
    }

    #[test]
    fn rejects_unknown_type() {
        let mut rec = DbSchemaRecord::new("shop", 1);
        rec.tables.push(table("users", vec![col("id", "bigint")]));
        let err = validate_record(&rec).unwrap_err();
        assert!(err.contains("unknown type"), "{err}");
        assert!(err.contains("integer"), "error lists allowed types: {err}");
    }

    #[test]
    fn rejects_fk_to_unknown_table() {
        let mut rec = DbSchemaRecord::new("shop", 1);
        rec.tables.push(table("users", vec![col("id", "integer")]));
        rec.fks.push(DesignFk {
            name: "".into(),
            from_table: "users".into(),
            from_columns: vec!["x".into()],
            to_table: "ghost".into(),
            to_columns: vec!["id".into()],
            on_delete: None,
        });
        assert!(validate_record(&rec).is_err());
    }
}
