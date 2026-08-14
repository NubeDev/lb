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
//!      AST at all — the table is chosen at runtime from caller-supplied `vars`. Any dynamic construct
//!      (a param, a function call, a block, a future, a cast, a model, a computed idiom) appearing in a
//!      **table position** (`FROM …`, a graph `->…`) is refused unless the request's own bindings
//!      resolve it: `$t` is *not* unknowable when the same call supplies `vars = {t: "site"}`, so we
//!      resolve it and judge the name. That keeps the wall's promise (the read is checked by table
//!      name) while letting the injection-safe parameterised form through — the platform `store-read`
//!      node builds exactly that shape on purpose, so that the author's table is a binding rather
//!      than spliced text.
//!
//! Two boundaries make the "table position" precise, and both matter:
//!   - it is **inherited** down a subtree, so a `type::table($t)` nested inside `FROM (…)` is still
//!     choosing a table;
//!   - it **ends at a nested statement**. `FROM (SELECT … FROM t WHERE …)` is a subquery whose own
//!     projection / `WHERE` / `ORDER BY` name no table, and whose `FROM` re-opens the position by its
//!     own key. Inheriting past that boundary refused every *composed* read in the product (each
//!     `Idiom` field reference inside the subquery read as a runtime-computed table).
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

/// The serde variant tag of a nested **statement**. A statement is its own scope: an enclosing table
/// position ends at its boundary, because the statement re-opens the positions it actually has (its
/// own `what`/`tb`). Only the read statement qualifies — anything else nested in a table position
/// keeps the strict treatment.
const NESTED_STATEMENT: &str = "Select";

/// The bound variables supplied with this same request (`store.query {sql, vars}`), used to *resolve*
/// a parameterised table position. See [`resolved_table`].
pub type Vars<'a> = &'a [(String, Value)];

/// Refuse `stmt` if it can read the secret plane — or if it cannot be *proven* not to. Runs after
/// `ensure_read_only`, on the same parsed statement, before any SQL reaches the store.
///
/// `vars` are the bindings that will be sent with the statement. They are part of the proof, not a
/// convenience: `FROM type::table($tb)` names no table in the AST, but when the same request binds
/// `$tb` to a string, the table it will read is known *here* and can be checked by name. Passing an
/// empty slice is always safe — it can only refuse more.
pub fn ensure_no_secret_read(stmt: &Statement, vars: Vars<'_>) -> Result<(), StoreQueryError> {
    // The AST is walked as serde data, not by matching every `sql::Value` variant by hand: the enum
    // is `#[non_exhaustive]` and grows, and a hand-written match silently stops covering a new
    // variant the day it lands — which here would be a credential leak, not a compile error.
    let tree = serde_json::to_value(stmt).map_err(|e| {
        StoreQueryError::Rejected(format!(
            "statement could not be inspected for secret-table access, so it is refused: {e}"
        ))
    })?;
    walk(&tree, false, vars)
}

fn walk(node: &Value, table_position: bool, vars: Vars<'_>) -> Result<(), StoreQueryError> {
    match node {
        Value::String(s) => {
            if let Some(t) = lb_store::secret_table_of(s) {
                return Err(refused(t));
            }
            Ok(())
        }
        Value::Array(items) => items.iter().try_for_each(|v| walk(v, table_position, vars)),
        Value::Object(map) => {
            // A dynamic node in a table position is refused for being *unprovable*, not for being
            // dynamic — so try to prove it first. When the request's own bindings say which table it
            // resolves to, that name is the thing to judge, and the subtree below it is just the
            // parameter name.
            if table_position {
                if let Some(name) = resolved_table(node, vars) {
                    return match lb_store::secret_table_of(&name) {
                        Some(t) => Err(refused(t)),
                        None => Ok(()),
                    };
                }
            }
            for (key, val) in map {
                if table_position && DYNAMIC_IN_TABLE_POSITION.contains(&key.as_str()) {
                    return Err(StoreQueryError::Rejected(format!(
                        "a table computed at run time ({key}) cannot be proven not to be a secret \
                         table, so the query is refused — name the table literally"
                    )));
                }
                // A table position is entered by key and stays open for the whole subtree — a nested
                // `type::table($t)` inside `FROM (…)` is still choosing a table to read — but it ENDS
                // at a nested statement. `FROM (SELECT …)` is a subquery, and inside it a field
                // reference, a `WHERE`, an `ORDER BY` are ordinary: they cannot name a table to read,
                // and the subquery's own `FROM` re-opens the position by its own `what` key. Without
                // this boundary every composed read (`SELECT * FROM (SELECT … FROM t) WHERE …`) is
                // refused for the `Idiom` of its own projection — see the debug entry.
                let inner = if key == NESTED_STATEMENT {
                    false
                } else {
                    table_position || TABLE_POSITION_KEYS.contains(&key.as_str())
                };
                walk(val, inner, vars)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

/// The literal table name a table-position node resolves to, when the request's bindings make that
/// knowable. `None` means "not provable here" — the caller then falls back to refusing.
///
/// Deliberately narrow: exactly the two shapes that carry a table through a binding —
/// `FROM $tb` and `FROM type::table(<literal|$tb>)`. Anything else (a computed expression, another
/// function, a block, an idiom) stays unprovable, because widening this is widening the wall.
fn resolved_table(node: &Value, vars: Vars<'_>) -> Option<String> {
    let map = node.as_object()?;
    if let Some(Value::String(param)) = map.get("Param") {
        return bound_string(param, vars);
    }
    // {"Function":{"Normal":["type::table",[<arg>]]}}
    let args = map
        .get("Function")?
        .get("Normal")?
        .as_array()
        .filter(|f| f.first().and_then(Value::as_str) == Some("type::table"))?
        .get(1)?
        .as_array()?;
    let [arg] = args.as_slice() else { return None };
    match arg.as_object()? {
        m if m.contains_key("Strand") => m.get("Strand")?.as_str().map(str::to_string),
        m => bound_string(m.get("Param")?.as_str()?, vars),
    }
}

/// The string a `$name` binding carries, if it is bound to a plain string in this request.
fn bound_string(name: &str, vars: Vars<'_>) -> Option<String> {
    vars.iter()
        .find(|(k, _)| k == name)
        .and_then(|(_, v)| v.as_str())
        .map(str::to_string)
}

/// The one refusal message, naming the table so the SQL editor can explain it. Safe to surface: the
/// caller wrote the name, so it carries no existence signal.
fn refused(table: &'static str) -> StoreQueryError {
    StoreQueryError::SecretTable(table)
}
