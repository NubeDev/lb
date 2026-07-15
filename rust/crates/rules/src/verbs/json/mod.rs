//! `json_*` + SurrealDB-shape helpers (data-stdlib-scope): parse/stringify, deep path get/set,
//! merge/flatten, and the row-shape verbs for rows as sources actually return them (record `Thing`
//! ids, ISO datetime strings, arrays of row maps). Pure compute — no seam, no cap, no I/O; the
//! family adds **zero authority** (it runs green with an empty allowlist and an empty cap set).
//!
//! Folder-of-verbs (FILE-LAYOUT): one concern per file —
//! `codec` (parse/stringify), `path` (deep-path get/set/has), `shape` (merge/flatten/pick/entries),
//! `rows` (array-of-row-maps verbs), `surreal` (Thing ids + epoch normalizers), `catalog` (the rows
//! `rules.help` returns). Missing-value policy everywhere: missing = `()` ↔ JSON `null`.

mod catalog;
mod codec;
mod path;
mod rows;
mod shape;
mod surreal;

pub(crate) use catalog::CATALOG;

use rhai::Engine;

/// Register the JSON/shape verbs (free functions — no handle).
pub fn register(engine: &mut Engine) {
    codec::register(engine);
    path::register(engine);
    shape::register(engine);
    rows::register(engine);
    surreal::register(engine);
}
