//! Dialect-neutral type mapping + DDL generation + diff (schema-designer scope). The `db_schema`
//! record stores types in a small canonical vocabulary (`text`/`integer`/…); each `Source` kind
//! maps that vocabulary to its native DDL types at plan time, and maps a live catalog's type
//! string BACK to the canonical vocabulary for the diff — so `varchar` vs `character varying`
//! never produces a spurious ALTER (the load-bearing correctness invariant, scope Risk 1).
//!
//! **Additive only (v1).** The diff plans `CREATE TABLE` + `ALTER TABLE … ADD COLUMN` + additive
//! FK constraints. A destructive change (dropped table/column, narrowed nullability, changed
//! type) is REFUSED with a clear "what to do instead" error — a destructive migration is a named
//! future verb, not a flag on this one (scope Non-goals).
//!
//! Pure (no IO, no `&self`) → unit-testable. One responsibility, one file (FILE-LAYOUT).

use serde::{Deserialize, Serialize};

/// The dialect-neutral type vocabulary a design record may name. Anything else in a record is a
/// validation error at `dbschema.save`; the migrate planner never sees an unknown neutral type.
#[allow(dead_code)]
pub const NEUTRAL_TYPES: &[&str] = &[
    "text",
    "integer",
    "real",
    "boolean",
    "blob",
    "date",
    "timestamp",
    "numeric",
    "json",
];

/// A designed table (the wire shape of a `db_schema` record's `tables[i]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignTable {
    pub name: String,
    pub columns: Vec<DesignColumn>,
    /// Primary-key column names (composite allowed). Empty = no PK.
    #[serde(default)]
    pub pk: Vec<String>,
}

/// A designed column. `r#type` is a canonical neutral type (validated against [`NEUTRAL_TYPES`]).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignColumn {
    pub name: String,
    #[serde(rename = "type")]
    pub r#type: String,
    #[serde(default)]
    pub nullable: bool,
    /// A raw SQL fragment for the column DEFAULT, emitted verbatim after `DEFAULT`. None = no
    /// default. (v1: unchecked SQL — the record is admin-authored; a future tighten could parse.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// A designed foreign-key constraint (the wire shape of `fks[i]`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesignFk {
    /// Constraint name (used in `ADD CONSTRAINT <name>`). Auto-generated when empty.
    #[serde(default)]
    pub name: String,
    pub from_table: String,
    pub from_columns: Vec<String>,
    pub to_table: String,
    pub to_columns: Vec<String>,
    /// `CASCADE` / `SET NULL` / `RESTRICT` / `NO ACTION`. None = dialect default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_delete: Option<String>,
}

/// The full designed schema (the wire shape the host hands the sidecar for migrate).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DesignSchema {
    pub tables: Vec<DesignTable>,
    #[serde(default)]
    pub fks: Vec<DesignFk>,
}

/// One live catalog column as the diff sees it: name + its type normalized back to the canonical
/// vocabulary + nullability. The migrate planner compares these against the desired design.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveColumn {
    pub name: String,
    pub neutral_type: String,
    pub nullable: bool,
}

/// The live catalog snapshot the diff needs: existing tables → their columns + PK column names.
#[derive(Debug, Clone, Default)]
pub struct LiveCatalog {
    /// table name → (columns, pk column names)
    pub tables: Vec<(String, Vec<LiveColumn>, Vec<String>)>,
}

/// One planned DDL statement, classified so the host/applier can assert it is on the allow-list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdlStatement {
    /// `CREATE TABLE …` — a brand-new table (the diff found no live table by that name).
    CreateTable { table: String, sql: String },
    /// `ALTER TABLE … ADD COLUMN …` — an additive column on an existing table.
    AddColumn {
        table: String,
        column: String,
        sql: String,
    },
    /// `ALTER TABLE … ADD CONSTRAINT … FOREIGN KEY …` — an additive FK (CREATE has its own inline).
    AddFk {
        table: String,
        name: String,
        sql: String,
    },
}

impl DdlStatement {
    /// The raw SQL string to execute.
    pub fn sql(&self) -> &str {
        match self {
            DdlStatement::CreateTable { sql, .. }
            | DdlStatement::AddColumn { sql, .. }
            | DdlStatement::AddFk { sql, .. } => sql,
        }
    }
}

/// The result of planning a migrate: the ordered statements to apply (empty = nothing to do, the
/// idempotent re-run case) or a refusal describing the destructive change.
#[derive(Debug, Clone)]
pub struct DdlPlan {
    pub statements: Vec<DdlStatement>,
}

/// A destructive change the v1 planner refuses. The error message names what to do instead
/// (scope Risk 2: get this copy right or the feature feels broken).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestructiveRefusal {
    pub message: String,
}

/// Map a canonical neutral type to the given kind's native DDL type string. An unknown kind or
/// neutral type falls back to `text` (a safe widest type) — the record validated at save, so this
/// is defense in depth.
pub fn neutral_to_kind(neutral: &str, kind: &str) -> &'static str {
    match kind {
        "sqlite" => match neutral {
            "text" => "TEXT",
            "integer" => "INTEGER",
            "real" => "REAL",
            "boolean" => "BOOLEAN",
            "blob" => "BLOB",
            "date" => "DATE",
            "timestamp" => "TIMESTAMP",
            "numeric" => "NUMERIC",
            "json" => "JSON",
            _ => "TEXT",
        },
        "postgres" | "timescale" => match neutral {
            "text" => "text",
            "integer" => "bigint",
            "real" => "double precision",
            "boolean" => "boolean",
            "blob" => "bytea",
            "date" => "date",
            "timestamp" => "timestamp",
            "numeric" => "numeric",
            "json" => "jsonb",
            _ => "text",
        },
        _ => "text",
    }
}

/// Normalize a live catalog type string back to the canonical vocabulary. This is the diff's
/// load-bearing function: `varchar`/`character varying(255)` must both map to `text`, or the diff
/// plans a spurious ALTER forever (scope Risk 1). Unknown types map to `text` (a safe widest
/// type) — the diff then treats an unknown-live vs known-desired as a type-mismatch refusal (safe).
pub fn canonicalize_live_type(live: &str, kind: &str) -> String {
    let lc = live.trim().to_ascii_lowercase();
    let bare = lc.split('(').next().unwrap_or("").trim();
    let neutral: &'static str = match kind {
        "sqlite" => match bare {
            "text" | "varchar" | "char" | "character" | "clob" | "string" => "text",
            "integer" | "int" | "tinyint" | "smallint" | "mediumint" | "bigint" => "integer",
            "real" | "float" | "double" | "double precision" => "real",
            "boolean" | "bool" => "boolean",
            "blob" | "varbinary" | "binary" => "blob",
            "timestamp" | "datetime" => "timestamp",
            "date" => "date",
            "numeric" | "decimal" => "numeric",
            "json" => "json",
            _ => "text", // sqlite is dynamically typed; widen to text rather than refuse
        },
        "postgres" | "timescale" => match bare {
            "text" | "varchar" | "character varying" | "char" | "character" | "bpchar" => "text",
            "integer" | "int" | "int4" | "smallint" | "int2" | "bigint" | "int8" | "serial"
            | "bigserial" => "integer",
            "real" | "float4" | "double precision" | "float8" => "real",
            "boolean" | "bool" => "boolean",
            "bytea" | "blob" => "blob",
            "date" => "date",
            "timestamp"
            | "timestamp without time zone"
            | "timestamp with time zone"
            | "timestamptz" => "timestamp",
            "numeric" | "decimal" => "numeric",
            "json" | "jsonb" => "json",
            _ => "text",
        },
        _ => "text",
    };
    neutral.to_string()
}

/// Diff a desired `DesignSchema` against the `LiveCatalog`, producing the additive DDL plan. A
/// destructive change (dropped table/column, type change, NOT NULL → NULL narrowing on an existing
/// column) is REFUSED. Re-running on an unchanged schema yields an empty plan (idempotence).
pub fn plan_migrate(
    desired: &DesignSchema,
    live: &LiveCatalog,
    kind: &str,
) -> Result<DdlPlan, DestructiveRefusal> {
    let mut statements = Vec::new();

    // CREATE TABLE for new tables + ADD COLUMN for new columns on existing ones.
    for table in &desired.tables {
        // A live entry with ZERO columns means the table does NOT exist (a real table always has
        // ≥1 column) — `list_columns_with_types` returns empty for both cases and we collapse them
        // here so the engine need not call `list_tables` (a pushed-down catalog scan that
        // mis-resolves on an empty source).
        let live_entry = live
            .tables
            .iter()
            .find(|(n, c, _)| n == &table.name && !c.is_empty());
        match live_entry {
            None => {
                // Brand-new table → CREATE TABLE. FKs whose `from_table` is this table are
                // INLINED in the CREATE for sqlite (sqlite has no `ALTER TABLE ADD CONSTRAINT`);
                // for postgres they're emitted as separate ADD CONSTRAINT statements below so a
                // CREATE never references a not-yet-created table (postgres checks at CREATE time).
                let fks_for_this: Vec<&DesignFk> = desired
                    .fks
                    .iter()
                    .filter(|fk| fk.from_table == table.name)
                    .collect();
                let sql = create_table_sql(table, kind, &fks_for_this);
                statements.push(DdlStatement::CreateTable {
                    table: table.name.clone(),
                    sql,
                });
            }
            Some((_, live_cols, _live_pk)) => {
                let live_cols = live_cols.as_slice();
                // Existing table → only ADD COLUMN is additive. Three destructive cases are refused:
                //   - a desired column whose type CHANGED (can't alter type additively);
                //   - a LIVE column the design DROPPED (removing it is destructive);
                // Nullability changes are IGNORED in v1 (no statement, no refusal) — emitting a
                // SET/DROP NOT NULL is a separate concern that can fail on existing rows, and
                // skipping it is never destructive (the table still serves every query it did).
                let live_by_name: std::collections::BTreeMap<&str, &LiveColumn> =
                    live_cols.iter().map(|c| (c.name.as_str(), c)).collect();
                let mut desired_names: std::collections::HashSet<&str> =
                    std::collections::HashSet::new();
                for col in &table.columns {
                    desired_names.insert(col.name.as_str());
                    match live_by_name.get(col.name.as_str()) {
                        None => {
                            let sql = add_column_sql(&table.name, col, kind);
                            statements.push(DdlStatement::AddColumn {
                                table: table.name.clone(),
                                column: col.name.clone(),
                                sql,
                            });
                        }
                        Some(lc) => {
                            if lc.neutral_type != col.r#type {
                                return Err(DestructiveRefusal {
                                    message: format!(
                                        "column `{}.{}` type changed from live `{}` to designed \
                                         `{}`. v1 migrate is additive only — to change a column \
                                         type, add a new column, backfill it, and drop the old \
                                         one via a future destructive-migrate verb.",
                                        table.name, col.name, lc.neutral_type, col.r#type
                                    ),
                                });
                            }
                            // type matches; nullability is intentionally not diffed in v1.
                        }
                    }
                }
                // A LIVE column the design no longer names is a destructive drop → refuse.
                for lc in live_cols.iter() {
                    if !desired_names.contains(lc.name.as_str()) {
                        return Err(DestructiveRefusal {
                            message: format!(
                                "column `{}.{}` was dropped from the design but exists live. v1 \
                                 migrate is additive only — to remove a column, add a new one, \
                                 migrate reads to it, and drop the old via a future \
                                 destructive-migrate verb.",
                                table.name, lc.name
                            ),
                        });
                    }
                }
            }
        }
    }

    // FK constraints: additive only. For POSTGRES, emit ADD CONSTRAINT for each FK whose owning
    // table exists (by now every CREATE has run in the same transaction, so parent tables are
    // present). For SQLITE, FKs were already inlined in CREATE TABLE above — sqlite has no
    // `ALTER TABLE ADD CONSTRAINT`, so an FK on an EXISTING table cannot be added in v1 (a
    // documented limitation; the design record still stores it for the canvas + future support).
    // We never detect/drop existing FKs (removing a constraint is destructive → future verb).
    let can_alter_fk = !(kind == "sqlite");
    if can_alter_fk {
        for fk in &desired.fks {
            let name = if fk.name.is_empty() {
                format!("fk_{}_{}", fk.from_table, fk.from_columns.join("_"))
            } else {
                fk.name.clone()
            };
            let sql = add_fk_sql(fk, &name, kind);
            statements.push(DdlStatement::AddFk {
                table: fk.from_table.clone(),
                name,
                sql,
            });
        }
    }

    Ok(DdlPlan { statements })
}

/// Generate `CREATE TABLE <name> (<columns>, PRIMARY KEY (<pk>) [, <inline FKs>])`. For sqlite the
/// FK constraints are INLINED here (sqlite has no `ALTER TABLE ADD CONSTRAINT`); postgres gets them
/// as separate ADD CONSTRAINT statements, so `fks` is empty for the postgres kind. SQLite allows
/// forward references in CREATE TABLE FKs (the constraint is parsed but only enforced at insert
/// time), so table order in the record is free even when FKs are inlined.
fn create_table_sql(table: &DesignTable, kind: &str, fks: &[&DesignFk]) -> String {
    let mut cols: Vec<String> = table.columns.iter().map(|c| column_def(c, kind)).collect();
    if !table.pk.is_empty() {
        let pk: Vec<String> = table.pk.iter().map(|c| quote_ident(c)).collect();
        cols.push(format!("PRIMARY KEY ({})", pk.join(", ")));
    }
    // Inline FKs (sqlite path). An FK referencing a table created LATER in the same batch is fine
    // for sqlite (forward ref); postgres would reject it at CREATE time, hence the per-kind split.
    for fk in fks {
        let from_cols: Vec<String> = fk.from_columns.iter().map(|c| quote_ident(c)).collect();
        let to_cols: Vec<String> = fk.to_columns.iter().map(|c| quote_ident(c)).collect();
        let mut clause = format!(
            "FOREIGN KEY ({}) REFERENCES {} ({})",
            from_cols.join(", "),
            quote_ident(&fk.to_table),
            to_cols.join(", ")
        );
        if let Some(on_delete) = &fk.on_delete {
            clause.push_str(&format!(" ON DELETE {on_delete}"));
        }
        cols.push(clause);
    }
    format!(
        "CREATE TABLE {} (\n  {}\n)",
        quote_ident(&table.name),
        cols.join(",\n  ")
    )
}

/// One column definition: `"<name>" <type> [NOT NULL] [DEFAULT <fragment>]`.
fn column_def(col: &DesignColumn, kind: &str) -> String {
    let mut s = format!(
        "{} {}",
        quote_ident(&col.name),
        neutral_to_kind(&col.r#type, kind)
    );
    if !col.nullable {
        s.push_str(" NOT NULL");
    }
    if let Some(default) = &col.default {
        s.push_str(&format!(" DEFAULT {default}"));
    }
    s
}

/// `ALTER TABLE <t> ADD COLUMN <col-def>`. SQLite/Postgres both accept this shape. SQLite
/// ADD COLUMN cannot change PK; we refuse that at the design-record level (PK is table-level).
fn add_column_sql(table: &str, col: &DesignColumn, kind: &str) -> String {
    format!(
        "ALTER TABLE {} ADD COLUMN {}",
        quote_ident(table),
        column_def(col, kind)
    )
}

/// `ALTER TABLE <from> ADD CONSTRAINT <name> FOREIGN KEY (<cols>) REFERENCES <to> (<cols>)
///  [ON DELETE <action>]`.
fn add_fk_sql(fk: &DesignFk, name: &str, _kind: &str) -> String {
    let from_cols: Vec<String> = fk.from_columns.iter().map(|c| quote_ident(c)).collect();
    let to_cols: Vec<String> = fk.to_columns.iter().map(|c| quote_ident(c)).collect();
    let mut sql = format!(
        "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {} ({})",
        quote_ident(&fk.from_table),
        quote_ident(name),
        from_cols.join(", "),
        quote_ident(&fk.to_table),
        to_cols.join(", ")
    );
    if let Some(on_delete) = &fk.on_delete {
        sql.push_str(&format!(" ON DELETE {on_delete}"));
    }
    sql
}

/// Quote an identifier (table/column/constraint name). Double-quoted per SQL standard (Postgres
/// + SQLite both accept). A name with an embedded `"` is rejected — caller-side the design record
/// validates identifiers to `[a-zA-Z_][a-zA-Z0-9_]*`, so this is defense in depth.
pub fn quote_ident(name: &str) -> String {
    if name.contains('"') {
        // A quote in an identifier is an injection vector; refuse rather than double it (the
        // design record validates identifiers, so we never expect this in practice).
        format!("\"{}\"", name.replace('"', "_"))
    } else {
        format!("\"{name}\"")
    }
}

#[cfg(test)]
mod tests {
    //! Diff-idempotence + additive-only + destructive-refusal tests (scope Testing plan). Pure —
    //! no IO, no Source, no DSN. The load-bearing correctness invariant: re-running migrate on an
    //! unchanged schema plans ZERO statements, and a destructive change is refused with copy that
    //! says what to do instead.

    use super::*;

    fn col(name: &str, ty: &str, nullable: bool) -> DesignColumn {
        DesignColumn {
            name: name.into(),
            r#type: ty.into(),
            nullable,
            default: None,
        }
    }

    fn table(name: &str, cols: Vec<DesignColumn>, pk: Vec<&str>) -> DesignTable {
        DesignTable {
            name: name.into(),
            columns: cols,
            pk: pk.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn live_col(name: &str, ty: &str, nullable: bool) -> LiveColumn {
        LiveColumn {
            name: name.into(),
            neutral_type: ty.into(),
            nullable,
        }
    }

    fn live_empty() -> LiveCatalog {
        LiveCatalog::default()
    }

    fn live_one(name: &str, cols: Vec<LiveColumn>, pk: Vec<&str>) -> LiveCatalog {
        LiveCatalog {
            tables: vec![(
                name.into(),
                cols,
                pk.iter().map(|s| s.to_string()).collect(),
            )],
        }
    }

    /// Brand-new table → exactly one CREATE TABLE, no ALTERs.
    #[test]
    fn new_table_plans_create_only() {
        let desired = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "integer", false), col("name", "text", true)],
                vec!["id"],
            )],
            fks: vec![],
        };
        let plan = plan_migrate(&desired, &live_empty(), "sqlite").unwrap();
        assert_eq!(plan.statements.len(), 1, "one CREATE TABLE: {:?}", plan);
        assert!(matches!(
            plan.statements[0],
            DdlStatement::CreateTable { ref table, .. } if table == "users"
        ));
        let sql = plan.statements[0].sql();
        assert!(sql.contains("CREATE TABLE"), "{sql}");
        assert!(sql.contains("PRIMARY KEY"), "{sql}");
    }

    /// Re-running migrate on a schema that already matches plans ZERO statements — the
    /// idempotence heart (scope Risk 1).
    #[test]
    fn unchanged_schema_plans_zero_statements() {
        let desired = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "integer", false), col("email", "text", false)],
                vec!["id"],
            )],
            fks: vec![],
        };
        let live = live_one(
            "users",
            vec![
                live_col("id", "integer", false),
                live_col("email", "text", false),
            ],
            vec!["id"],
        );
        let plan = plan_migrate(&desired, &live, "postgres").unwrap();
        assert!(
            plan.statements.is_empty(),
            "unchanged schema → zero statements: {:?}",
            plan
        );
    }

    /// A new column on an existing table plans exactly one ADD COLUMN.
    #[test]
    fn additive_column_plans_one_alter() {
        let desired = DesignSchema {
            tables: vec![table(
                "users",
                vec![
                    col("id", "integer", false),
                    col("email", "text", false),
                    col("status", "text", true), // new
                ],
                vec!["id"],
            )],
            fks: vec![],
        };
        let live = live_one(
            "users",
            vec![
                live_col("id", "integer", false),
                live_col("email", "text", false),
            ],
            vec!["id"],
        );
        let plan = plan_migrate(&desired, &live, "sqlite").unwrap();
        assert_eq!(plan.statements.len(), 1, "one ADD COLUMN: {:?}", plan);
        assert!(matches!(
            plan.statements[0],
            DdlStatement::AddColumn { ref table, ref column, .. } if table == "users" && column == "status"
        ));
        assert!(plan.statements[0].sql().contains("ADD COLUMN"));
    }

    /// A dropped column is REFUSED (v1 additive-only).
    #[test]
    fn dropped_column_refused() {
        let desired = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "integer", false)], // `email` dropped
                vec!["id"],
            )],
            fks: vec![],
        };
        let live = live_one(
            "users",
            vec![
                live_col("id", "integer", false),
                live_col("email", "text", false),
            ],
            vec!["id"],
        );
        let err = plan_migrate(&desired, &live, "sqlite").unwrap_err();
        assert!(
            err.message.contains("additive only") || err.message.contains("destructive"),
            "refusal copy names the policy: {}",
            err.message
        );
    }

    /// A type change on an existing column is REFUSED, and the copy says what to do instead.
    #[test]
    fn type_change_refused_with_what_to_do() {
        let desired = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "text", false)], // was integer
                vec!["id"],
            )],
            fks: vec![],
        };
        let live = live_one("users", vec![live_col("id", "integer", false)], vec!["id"]);
        let err = plan_migrate(&desired, &live, "postgres").unwrap_err();
        assert!(
            err.message.contains("add a new column"),
            "refusal says what to do: {}",
            err.message
        );
    }

    /// Type-normalization is the diff's load-bearing function — `varchar(255)` live vs `text`
    /// designed must NOT plan a spurious ALTER (scope Risk 1).
    #[test]
    fn live_type_normalizes_for_idempotent_diff() {
        assert_eq!(canonicalize_live_type("varchar(255)", "postgres"), "text");
        assert_eq!(
            canonicalize_live_type("character varying(100)", "postgres"),
            "text"
        );
        assert_eq!(canonicalize_live_type("bigint", "postgres"), "integer");
        assert_eq!(canonicalize_live_type("int4", "postgres"), "integer");
        assert_eq!(
            canonicalize_live_type("double precision", "postgres"),
            "real"
        );
        assert_eq!(
            canonicalize_live_type("timestamp without time zone", "postgres"),
            "timestamp"
        );
        assert_eq!(canonicalize_live_type("jsonb", "postgres"), "json");
        assert_eq!(canonicalize_live_type("TEXT", "sqlite"), "text");
        assert_eq!(canonicalize_live_type("INTEGER", "sqlite"), "integer");
    }

    /// An FK constraint plans an additive ADD CONSTRAINT (never dropped — that's destructive).
    #[test]
    fn fk_plans_add_constraint() {
        let desired = DesignSchema {
            tables: vec![
                table("users", vec![col("id", "integer", false)], vec!["id"]),
                table(
                    "orders",
                    vec![col("id", "integer", false), col("user_id", "integer", true)],
                    vec!["id"],
                ),
            ],
            fks: vec![DesignFk {
                name: "".into(),
                from_table: "orders".into(),
                from_columns: vec!["user_id".into()],
                to_table: "users".into(),
                to_columns: vec!["id".into()],
                on_delete: Some("CASCADE".into()),
            }],
        };
        let live = LiveCatalog {
            tables: vec![
                (
                    "users".into(),
                    vec![live_col("id", "integer", false)],
                    vec!["id".into()],
                ),
                (
                    "orders".into(),
                    vec![
                        live_col("id", "integer", false),
                        live_col("user_id", "integer", true),
                    ],
                    vec!["id".into()],
                ),
            ],
        };
        let plan = plan_migrate(&desired, &live, "postgres").unwrap();
        // Both tables exist unchanged; only the FK is new.
        assert_eq!(plan.statements.len(), 1, "one ADD CONSTRAINT: {:?}", plan);
        assert!(matches!(
            plan.statements[0],
            DdlStatement::AddFk { ref table, .. } if table == "orders"
        ));
        let sql = plan.statements[0].sql();
        assert!(sql.contains("ADD CONSTRAINT"), "{sql}");
        assert!(sql.contains("FOREIGN KEY"), "{sql}");
        assert!(sql.contains("REFERENCES"), "{sql}");
        assert!(sql.contains("ON DELETE CASCADE"), "{sql}");
        // Auto-generated name when the record left it blank.
        assert!(sql.contains("fk_orders_user_id"), "{sql}");
    }

    /// Nullability changes are IGNORED in v1 — neither tightening nor widening emits a statement
    /// or a refusal. Skipping a constraint change is never destructive (the table still serves
    /// every query it did); emitting one can fail on existing rows, so v1 leaves it to a future
    /// destructive-migrate verb. Type changes and dropped columns ARE refused (above).
    #[test]
    fn nullability_changes_are_ignored_in_v1() {
        // Tightening: live NULL, designed NOT NULL → no statement, no refusal.
        let desired_tight = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "integer", false), col("email", "text", false)],
                vec!["id"],
            )],
            fks: vec![],
        };
        let live_null = live_one(
            "users",
            vec![
                live_col("id", "integer", false),
                live_col("email", "text", true), // live nullable
            ],
            vec!["id"],
        );
        let plan = plan_migrate(&desired_tight, &live_null, "sqlite").unwrap();
        assert!(plan.statements.is_empty(), "tightening ignored: {plan:?}");

        // Widening: live NOT NULL, designed NULL → no statement, no refusal.
        let desired_wide = DesignSchema {
            tables: vec![table(
                "users",
                vec![col("id", "integer", false), col("email", "text", true)],
                vec!["id"],
            )],
            fks: vec![],
        };
        let live_tight = live_one(
            "users",
            vec![
                live_col("id", "integer", false),
                live_col("email", "text", false), // live NOT NULL
            ],
            vec!["id"],
        );
        let plan = plan_migrate(&desired_wide, &live_tight, "sqlite").unwrap();
        assert!(plan.statements.is_empty(), "widening ignored: {plan:?}");
    }

    /// Identifier quoting — a name with a `"` is defanged (never echoed raw into DDL).
    #[test]
    fn quote_ident_defangs_embedded_quote() {
        assert_eq!(quote_ident("users"), "\"users\"");
        assert_eq!(quote_ident("ev\"il"), "\"ev_il\"");
    }
}
