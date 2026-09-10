//! Composing the `store-read` node's SELECT — the only place this pack builds SQL.
//!
//! Split out of `store_crud.rs` so the node file stays about DISPATCH (config precedence, gates,
//! outcome shaping) and this one stays about the statement. The injection posture lives here: the
//! table rides as a `type::table($tb)` binding, every caller VALUE is `$`-bound, and the only text
//! ever spliced in is an identifier-checked field name or a host-clamped integer.

use serde_json::{json, Map, Value};

/// The read limit's default / hard max (the scope table: default 100, max 1000).
const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 1000;

/// A store-legal identifier: `[A-Za-z_][A-Za-z0-9_]*`. Anything else is rejected BEFORE dispatch —
/// table, filter-field, and order-by names are the only text spliced into the built SELECT.
pub(super) fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// The alias the ordered path is projected under so SurrealDB 3's "order idiom must be selected"
/// rule is satisfied. Leading underscore and a name no caller writes, because it is `OMIT`ed and
/// must never collide with a real field.
const ORDER_ALIAS: &str = "_ord";

/// Build the parameterized SELECT for a `store-read`: `(sql, vars)` ready for `store.query`'s
/// `{sql, vars}` args. Every value is `$`-bound; the spliced text is limited to identifier-checked
/// field names and the clamped integer limit.
pub(super) fn build_read_sql(
    table: &str,
    id: Option<&str>,
    filter: Option<&Map<String, Value>>,
    limit: Option<u64>,
    order_by: Option<&str>,
    desc: bool,
) -> Result<(String, Map<String, Value>), String> {
    if !is_ident(table) {
        return Err(format!("invalid table identifier: {table}"));
    }
    let mut vars = Map::new();
    vars.insert("tb".into(), json!(table));
    let mut conds: Vec<String> = Vec::new();
    if let Some(id) = id {
        conds.push("record::id(id) = $id".into());
        vars.insert("id".into(), json!(id));
    }
    if let Some(filter) = filter {
        for (i, (field, value)) in filter.iter().enumerate() {
            if !is_ident(field) {
                return Err(format!("invalid filter field identifier: {field}"));
            }
            conds.push(format!("data.{field} = $f{i}"));
            vars.insert(format!("f{i}"), value.clone());
        }
    }
    // Project explicitly: the raw record `id` is a SurrealDB Thing, which does not serialize to
    // JSON through `store.query`'s row decode — `record::id(id)` yields the plain id string.
    //
    // ORDERING, and why the projection grows a helper column. SurrealDB requires the ORDER BY idiom
    // to appear in the statement's selection, matching a selected field or alias EXACTLY (the prefix
    // relaxation is GROUP BY only). We select the `data` envelope and order by a path inside it, so
    // a plain `ORDER BY data.<field>` is a PARSE error — "Missing order idiom ... in selection".
    //
    // NOT a SurrealDB 3 change: `debugging/store/order-by-needs-selected-idiom.md` hit the same rule
    // on 2.x in 2026-06. What is new is noticing it here — the emitted SQL was only asserted as a
    // string, never executed, so this ordering path had been broken the whole time.
    //
    // That note weighed `data.ts AS ts` and rejected it for polluting a generic verb with a
    // caller-specific column, choosing instead to sort in the layer that owns the shape. That
    // reasoning holds for `lb_store::list`, which is handed an opaque `data` and must not guess
    // where the order key lives. It does not bind here: this is the flow `store-read` node, whose
    // `order_by` is named BY THE CALLER, so the shape is declared rather than assumed. And `OMIT`
    // answers the pollution objection outright — the alias never reaches the caller's row.
    //
    // `store/tests/order_by_idiom_probe.rs` pins all of it on the real engine, including that OMIT
    // still hides the alias after the sort.
    let order = match order_by {
        None => None,
        Some(field) => {
            if !is_ident(field) {
                return Err(format!("invalid order_by identifier: {field}"));
            }
            Some(field)
        }
    };
    let mut sql = String::from("SELECT record::id(id) AS id, data, rev");
    if let Some(field) = order {
        sql.push_str(&format!(
            ", data.{field} AS {ORDER_ALIAS} OMIT {ORDER_ALIAS}"
        ));
    }
    sql.push_str(" FROM type::table($tb)");
    if !conds.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conds.join(" AND "));
    }
    if order.is_some() {
        sql.push_str(&format!(" ORDER BY {ORDER_ALIAS}"));
        if desc {
            sql.push_str(" DESC");
        }
    }
    let limit = limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    sql.push_str(&format!(" LIMIT {limit}"));
    Ok((sql, vars))
}
