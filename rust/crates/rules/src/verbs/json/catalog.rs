//! Catalog rows for the `json` family — one row per verb in this folder; overload arities share
//! the row (the catalog's `names_are_unique` contract). Append-only; never reorder existing rows.

use crate::catalog::FnEntry;

pub(crate) const CATALOG: &[FnEntry] = &[
    // ---- codec.rs ----
    FnEntry {
        name: "parse_json",
        family: "json",
        signature: "parse_json(s: String) -> Dynamic",
        description: "Parse a JSON string into a map/array/scalar (JSON null becomes ()).",
    },
    FnEntry {
        name: "to_json",
        family: "json",
        signature: "to_json(v: Dynamic) -> String",
        description: "Serialize any value to a compact JSON string (() becomes null).",
    },
    FnEntry {
        name: "to_json_pretty",
        family: "json",
        signature: "to_json_pretty(v: Dynamic) -> String",
        description: "Serialize any value to an indented JSON string for readable message bodies.",
    },
    // ---- path.rs ----
    FnEntry {
        name: "jget",
        family: "json",
        signature: "jget(v: Dynamic, path: String) -> Dynamic  |  jget(v, path, default) -> Dynamic",
        description: "Deep-path get with \"a.b[0].c\" syntax; an absent path yields () (or the given default) — never throws.",
    },
    FnEntry {
        name: "jset",
        family: "json",
        signature: "jset(v: Dynamic, path: String, val: Dynamic) -> Dynamic",
        description: "Deep-path set returning a NEW value; missing maps are created and an index equal to the array length appends.",
    },
    FnEntry {
        name: "jhas",
        family: "json",
        signature: "jhas(v: Dynamic, path: String) -> bool",
        description: "True when the deep path is present (even with a () value).",
    },
    // ---- shape.rs ----
    FnEntry {
        name: "merge",
        family: "json",
        signature: "merge(a: Map, b: Map) -> Map",
        description: "RFC-7386-style deep merge where b wins and a () value in b deletes the key.",
    },
    FnEntry {
        name: "flatten",
        family: "json",
        signature: "flatten(map: Map, sep: String) -> Map",
        description: "Collapse nested maps into separator-joined keys (\"a.b.c\").",
    },
    FnEntry {
        name: "unflatten",
        family: "json",
        signature: "unflatten(map: Map, sep: String) -> Map",
        description: "Rebuild nesting from separator-joined keys (the inverse of flatten).",
    },
    FnEntry {
        name: "pick",
        family: "json",
        signature: "pick(map: Map, keys: Array) -> Map",
        description: "Keep only the named keys (absent keys are simply not present).",
    },
    FnEntry {
        name: "omit",
        family: "json",
        signature: "omit(map: Map, keys: Array) -> Map",
        description: "Drop the named keys and keep everything else.",
    },
    FnEntry {
        name: "entries",
        family: "json",
        signature: "entries(map: Map) -> Array",
        description: "Map to an array of [key, value] pairs (key-sorted, deterministic).",
    },
    FnEntry {
        name: "from_entries",
        family: "json",
        signature: "from_entries(pairs: Array) -> Map",
        description: "Array of [key, value] pairs back to a map (the inverse of entries).",
    },
    // ---- rows.rs ----
    FnEntry {
        name: "pluck",
        family: "json",
        signature: "pluck(rows: Array, field: String) -> Array",
        description: "Array of row maps to an array of the named field, () where a row lacks it.",
    },
    FnEntry {
        name: "index_by",
        family: "json",
        signature: "index_by(rows: Array, key: String) -> Map",
        description: "Rows to a map keyed by the field's string value; the last duplicate wins.",
    },
    FnEntry {
        name: "group_rows",
        family: "json",
        signature: "group_rows(rows: Array, key: String) -> Map",
        description: "Rows to a map of arrays, one bucket per distinct field value.",
    },
    FnEntry {
        name: "where_eq",
        family: "json",
        signature: "where_eq(rows: Array, key: String, val: Dynamic) -> Array",
        description: "Keep the rows whose field equals val (numbers compare across int/float).",
    },
    FnEntry {
        name: "sort_by",
        family: "json",
        signature: "sort_by(rows: Array, key: String) -> Array  |  sort_by(rows, key, desc: bool) -> Array",
        description: "Stable sort of rows by the field — ascending, or descending when desc is true (missing values first).",
    },
    FnEntry {
        name: "uniq_by",
        family: "json",
        signature: "uniq_by(rows: Array, key: String) -> Array",
        description: "Keep the FIRST row per distinct field value.",
    },
    FnEntry {
        name: "count_by",
        family: "json",
        signature: "count_by(rows: Array, key: String) -> Map",
        description: "Frequency map: distinct field value -> row count.",
    },
    // ---- surreal.rs ----
    FnEntry {
        name: "thing_id",
        family: "json",
        signature: "thing_id(thing: String) -> String",
        description: "The id half of a SurrealDB record id (\"sensor:abc\" -> \"abc\"), unescaping the angle-bracket form.",
    },
    FnEntry {
        name: "thing_tbl",
        family: "json",
        signature: "thing_tbl(thing: String) -> String",
        description: "The table half of a SurrealDB record id (\"sensor:abc\" -> \"sensor\").",
    },
    FnEntry {
        name: "epoch",
        family: "json",
        signature: "epoch(v: Dynamic) -> i64",
        description: "Normalize whatever timestamp the source returned (ISO-8601 string, epoch-secs or epoch-ms, number or string) to epoch seconds.",
    },
    FnEntry {
        name: "rows_epoch",
        family: "json",
        signature: "rows_epoch(rows: Array, field: String) -> Array",
        description: "Normalize the named ts field to epoch seconds across every row.",
    },
];
