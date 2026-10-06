//! Live catalog type names → the neutral vocabulary the migrate diff compares in. Split out of
//! `dialect.rs` (FILE-LAYOUT): this is the one mapping both engines' catalog reads share.

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
