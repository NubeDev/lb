//! The **raw-read wall over the secret plane** for SurrealQL (node-update scope, decision 9). The
//! owner gate on `secret.get` is only as strong as every *other* way to read the store, and
//! `store.query` is one: its cap `mcp:store.query:call` sits in the author-tier bundle, so without
//! this file `SELECT * FROM secret` hands a plain member the sealed value in plaintext, straight past
//! the owner wall. `parse.rs` bounds the statement's KIND (a single read); this file bounds what that
//! read may TOUCH.
//!
//! It applies to **every principal** — member, workspace admin, extension, and the host's own MCP
//! surfaces alike. There is no override capability, by design (rule 10: no caller is special-cased).
//! The list of tables is `lb_store::secret_tables::SECRET_TABLES`, the same slice `snapshot_guard`
//! refuses to copy — one list, two surfaces.
//!
//! ## How the proof works, and what it cannot prove
//!
//! The check runs on the **parsed AST**, never on the SQL text — a substring blocklist is defeated by
//! whitespace, a comment, a quoted identifier or an alias, and it over-matches on any string that
//! merely contains the word. We serialise the parsed [`Statement`] with serde and walk that tree,
//! which gives two structural signals:
//!
//!   1. **A secret-plane name anywhere in the statement.** Post-parse, the tree contains only what the
//!      parser recognised, so `FROM secret`, `FROM secret:abc`, an aliased/`AS`-projected read, a
//!      correlated subquery, a JOIN-shaped multi-table `FROM a, secret`, a graph part `->secret`, and
//!      a literal `type::table("secret")` all surface the same string. This is deliberately wider than
//!      a table position: a *string literal* equal to a secret table name (`WHERE kind = 'secret'`)
//!      is also refused. That is a **known false refusal** — cheap and visible — accepted because
//!      narrowing it would mean trusting our own reading of which AST slots can become a table.
//!   2. **A table position we cannot evaluate.** `SELECT * FROM type::table($t)` names no table in the
//!      AST at all — the table is chosen at runtime from caller-supplied `vars`. Nothing static can
//!      prove it is not `secret`, so any dynamic construct (a param, a function call, a block, a
//!      future, a cast, a model, a computed idiom) appearing in a **table position** (`FROM …`, a
//!      graph `->…`) is refused outright.
//!
//! Honestly stated limits: we do **not** prove anything about what the store then does with the rows
//! — this is a wall on the request, not row-level filtering; and if the serde tree of a statement ever
//! failed to build, we refuse rather than pass (the `Err` arm below). `INFO`/`SHOW` go through the
//! same walk, so `INFO FOR TABLE secret` is refused too — it discloses the shape of the secret plane.

use serde_json::Value;
use surrealdb::sql::Statement;

use super::error::StoreQueryError;

/// AST nodes whose value is only known at run time. In a **table position** none of these can be
/// statically proven not to resolve to a secret table, so their presence there is a refusal. (In a
/// projection or a `WHERE` they are ordinary and allowed — they cannot name a table to read.)
const DYNAMIC_IN_TABLE_POSITION: &[&str] = &[
    "Param",      // FROM $t  /  type::table($t)
    "Function",   // FROM type::table(...), FROM fn::whatever()
    "Model",      // FROM ml::model(...)
    "Block",      // FROM { ... }
    "Future",     // FROM <future> { ... }
    "Cast",       // FROM <record> $x
    "Expression", // FROM a ?: b
    "Idiom",      // FROM some.field  — resolves against the document, not a literal table
    "Mock",       // FROM |secret:1..10|  — generated ids over a runtime-shaped table
];

/// The serde keys that open a **table position** — the slot in a statement where a value is resolved
/// to a table to read. `what` is `SELECT … FROM <what>` and a graph part's `->(<what>)`; `tb` is a
/// record id's table. Everything reachable under one of these is judged by the stricter rule.
const TABLE_POSITION_KEYS: &[&str] = &["what", "tb", "table", "tables"];

/// Refuse `stmt` if it can read the secret plane — or if it cannot be *proven* not to. Runs after
/// `ensure_read_only`, on the same parsed statement, before any SQL reaches the store.
pub fn ensure_no_secret_read(stmt: &Statement) -> Result<(), StoreQueryError> {
    // The AST is walked as serde data, not by matching every `sql::Value` variant by hand: the enum
    // is `#[non_exhaustive]` and grows, and a hand-written match silently stops covering a new
    // variant the day it lands — which here would be a credential leak, not a compile error.
    let tree = serde_json::to_value(stmt).map_err(|e| {
        StoreQueryError::Rejected(format!(
            "statement could not be inspected for secret-table access, so it is refused: {e}"
        ))
    })?;
    walk(&tree, false)
}

fn walk(node: &Value, table_position: bool) -> Result<(), StoreQueryError> {
    match node {
        Value::String(s) => {
            if let Some(t) = lb_store::secret_table_of(s) {
                return Err(refused(t));
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(|v| walk(v, table_position)),
        Value::Object(map) => {
            for (key, val) in map {
                if table_position && DYNAMIC_IN_TABLE_POSITION.contains(&key.as_str()) {
                    return Err(StoreQueryError::Rejected(format!(
                        "a table computed at run time ({key}) cannot be proven not to be a secret \
                         table, so the query is refused — name the table literally"
                    )));
                }
                // A table position is entered by key and stays open for the whole subtree: a nested
                // `type::table($t)` inside `FROM (…)` is still choosing a table to read.
                let inner = table_position || TABLE_POSITION_KEYS.contains(&key.as_str());
                walk(val, inner)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The one refusal message, naming the table so the SQL editor can explain it. Safe to surface: the
/// caller wrote the name, so it carries no existence signal.
fn refused(table: &'static str) -> StoreQueryError {
    StoreQueryError::SecretTable(table)
}
