//! The entity→table **binding** lint (`pack-entity-binding-scope.md` §"The validation"). A pure
//! check that an entity's `{table, pk, parent_fk, display}` binding is well-formed against the pack's
//! OWN `schema.sql` DDL — the same text the dialect lint already reads.
//!
//! The posture mirrors the dialect poison list (`validate.rs` module doc): a binding that references
//! a table/column the schema does not declare is a **warning, not a gate**, because the schema can be
//! opaque (a `postgres` pack registers a pointer to a table this node never sees the DDL for) and the
//! real oracle is applying against the real source. The ONE structural error is `parent_fk` without a
//! declared `parent`: that is self-inconsistent in the manifest itself, readable with no schema at all
//! — exactly the class of the "dangling entity parent" error next to it.
//!
//! This stores nothing and generates no SQL: it is an *address* checker, holding the line
//! `pack-core-scope` drew (the binding is a projection, not behavior). The DDL scan is intentionally
//! small and forgiving — it recognizes `CREATE TABLE <name> ( <col> … )` well enough to catch the
//! common author typo, and stays silent (warns nothing) on anything it cannot parse, because a false
//! "missing table" warning on a valid pack is worse than a missed one the apply will surface anyway.

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::Entity;

/// A parsed view of a pack's DDL: table name → its declared column set. Absent tables/opaque schema
/// simply don't appear, and the caller treats "not found here" as *opaque*, never as *proven absent*.
#[derive(Debug, Default)]
pub struct SchemaTables(BTreeMap<String, BTreeSet<String>>);

impl SchemaTables {
    /// Parse `CREATE TABLE` statements out of a pack's schema SQL. Best-effort and dialect-blind: it
    /// finds each `create table <ident> ( … )` and pulls the leading identifier of every top-level
    /// comma-separated item as a column name (table-level constraints like `PRIMARY KEY (…)` lead
    /// with a keyword, which simply registers a harmless pseudo-column the checks never look up). A
    /// statement it cannot read is skipped — the table stays opaque, which the checks treat as "warn
    /// nothing", never "column missing".
    pub fn parse(schema_sql: &str) -> Self {
        let mut tables = BTreeMap::new();
        let lower = schema_sql.to_lowercase();
        let bytes = lower.as_bytes();
        let mut search = 0usize;
        while let Some(rel) = lower[search..].find("create table") {
            let stmt_start = search + rel + "create table".len();
            // The table name: the next identifier (skip `if not exists`).
            let mut i = skip_ws(bytes, stmt_start);
            if lower[i..].starts_with("if not exists") {
                i = skip_ws(bytes, i + "if not exists".len());
            }
            let (name, after_name) = read_ident(bytes, i);
            search = after_name.max(stmt_start + 1);
            let Some(name) = name else { continue };
            // The column list is between the matching parens after the name.
            let Some(open) = lower[after_name..].find('(').map(|p| after_name + p) else {
                continue;
            };
            let Some(close) = match_paren(bytes, open) else {
                continue;
            };
            let cols = parse_columns(&lower[open + 1..close]);
            tables.insert(name, cols);
        }
        Self(tables)
    }

    /// Does the schema declare `table`? `None` when the schema is opaque about it (parsed nothing for
    /// that name) — the caller must NOT treat that as "table absent".
    fn table(&self, table: &str) -> Option<&BTreeSet<String>> {
        self.0.get(&table.to_lowercase())
    }
}

/// Lint one entity's binding against `schema` and the entity's own shape. Returns `(errors, warnings)`
/// as message strings — the caller (`validate`) wraps them into `Finding`s. An entity with no `table`
/// yields nothing (the shape-only promise). Pure and total: no I/O, no panics.
pub fn validate_binding(
    name: &str,
    ent: &Entity,
    schema: &SchemaTables,
) -> (Vec<String>, Vec<String>) {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // `parent_fk` requires a declared parent — a manifest-only inconsistency, readable with no schema,
    // so it GATES like the dangling-parent error.
    if ent.parent_fk.is_some() && ent.parent.is_none() {
        errors.push(format!(
            "entity '{name}' sets parent_fk but declares no parent — a parent_fk with no parent to \
             reference is meaningless"
        ));
    }
    // pk/parent_fk/display without a table are inert — warn so the author isn't surprised the binding
    // does nothing.
    if ent.table.is_none() && (ent.pk.is_some() || ent.parent_fk.is_some() || ent.display.is_some())
    {
        warnings.push(format!(
            "entity '{name}' sets pk/parent_fk/display but no table — the binding is inert without a \
             table"
        ));
    }

    let Some(table) = &ent.table else {
        return (errors, warnings);
    };

    // Everything below is a WARNING: where the schema is opaque we cannot prove absence, and the real
    // oracle is the apply (the dialect-lint precedent).
    let Some(cols) = schema.table(table) else {
        warnings.push(format!(
            "entity '{name}' binds table '{table}', which the pack's schema does not declare — verify \
             against a real apply (the schema may be external/opaque)"
        ));
        return (errors, warnings);
    };
    for (field, col) in [
        ("pk", &ent.pk),
        ("parent_fk", &ent.parent_fk),
        ("display", &ent.display),
    ] {
        if let Some(col) = col {
            if !cols.contains(&col.to_lowercase()) {
                warnings.push(format!(
                    "entity '{name}' binds {field} '{col}', which is not a column of table '{table}'"
                ));
            }
        }
    }
    (errors, warnings)
}

// ----- small dialect-blind DDL scanning ---------------------------------------------------------

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

/// Read a SQL identifier at `i` (optionally quoted with `"`), returning it lowercased + the index
/// after it. `None` when `i` is not on an identifier.
fn read_ident(b: &[u8], i: usize) -> (Option<String>, usize) {
    let i = skip_ws(b, i);
    if i >= b.len() {
        return (None, i);
    }
    if b[i] == b'"' {
        // Quoted identifier: to the closing quote.
        let mut j = i + 1;
        while j < b.len() && b[j] != b'"' {
            j += 1;
        }
        let name: String = b[i + 1..j.min(b.len())]
            .iter()
            .map(|&c| c as char)
            .collect();
        return (Some(name), (j + 1).min(b.len()));
    }
    let start = i;
    let mut j = i;
    while j < b.len() && (b[j].is_ascii_alphanumeric() || b[j] == b'_') {
        j += 1;
    }
    if j == start {
        return (None, i);
    }
    let name: String = b[start..j].iter().map(|&c| c as char).collect();
    (Some(name), j)
}

/// Given `open` at a `(`, find the index of the matching `)`, respecting nesting. `None` if unbalanced.
fn match_paren(b: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut i = open;
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Pull the leading identifier of each top-level comma-separated item in a column list body. A
/// table-level constraint (`primary key (a, b)`, `foreign key …`) leads with a keyword and registers
/// a harmless pseudo-column no check ever looks up.
fn parse_columns(body: &str) -> BTreeSet<String> {
    let b = body.as_bytes();
    let mut cols = BTreeSet::new();
    let mut item_start = 0usize;
    let mut depth = 0i32;
    let mut i = 0usize;
    let push = |slice: &str, cols: &mut BTreeSet<String>| {
        let (ident, _) = read_ident(slice.as_bytes(), 0);
        if let Some(name) = ident {
            if !name.is_empty() {
                cols.insert(name.to_lowercase());
            }
        }
    };
    while i < b.len() {
        match b[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => {
                push(&body[item_start..i], &mut cols);
                item_start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    push(&body[item_start..], &mut cols);
    cols
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(
        parent: Option<&str>,
        table: Option<&str>,
        pk: Option<&str>,
        parent_fk: Option<&str>,
        display: Option<&str>,
    ) -> Entity {
        Entity {
            label: "L".into(),
            parent: parent.map(String::from),
            kinds: vec![],
            units: Default::default(),
            table: table.map(String::from),
            pk: pk.map(String::from),
            parent_fk: parent_fk.map(String::from),
            display: display.map(String::from),
            backend: None,
            geo: None,
        }
    }

    const BAS_SCHEMA: &str = "CREATE TABLE site (id TEXT PRIMARY KEY, name TEXT NOT NULL, lat REAL, lng REAL);\
        CREATE TABLE meter (id TEXT PRIMARY KEY, site_id TEXT NOT NULL REFERENCES site(id), name TEXT);\
        CREATE TABLE point (id TEXT PRIMARY KEY, meter_id TEXT NOT NULL REFERENCES meter(id), name TEXT);";

    #[test]
    fn parses_tables_and_columns() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let site = s.table("site").expect("site parsed");
        assert!(site.contains("id") && site.contains("name") && site.contains("lat"));
        let meter = s.table("meter").expect("meter parsed");
        assert!(meter.contains("site_id"));
    }

    #[test]
    fn a_clean_binding_warns_nothing() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (e, w) = validate_binding(
            "meter",
            &ent(
                Some("site"),
                Some("meter"),
                Some("id"),
                Some("site_id"),
                Some("name"),
            ),
            &s,
        );
        assert!(e.is_empty() && w.is_empty(), "e={e:?} w={w:?}");
    }

    #[test]
    fn parent_fk_without_parent_is_an_error() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (e, _) = validate_binding(
            "meter",
            &ent(None, Some("meter"), Some("id"), Some("site_id"), None),
            &s,
        );
        assert!(
            e.iter()
                .any(|m| m.contains("parent_fk") && m.contains("no parent")),
            "{e:?}"
        );
    }

    #[test]
    fn an_unknown_table_warns_but_does_not_error() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (e, w) = validate_binding(
            "ghost",
            &ent(None, Some("nope"), Some("id"), None, None),
            &s,
        );
        assert!(e.is_empty(), "unknown table must not gate: {e:?}");
        assert!(w.iter().any(|m| m.contains("nope")), "{w:?}");
    }

    #[test]
    fn an_unknown_column_warns() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (_, w) = validate_binding(
            "site",
            &ent(None, Some("site"), Some("uuid"), None, Some("label")),
            &s,
        );
        assert!(w.iter().any(|m| m.contains("uuid")), "{w:?}");
        assert!(w.iter().any(|m| m.contains("label")), "{w:?}");
    }

    #[test]
    fn no_table_is_the_shape_only_promise() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (e, w) = validate_binding("site", &ent(None, None, None, None, None), &s);
        assert!(
            e.is_empty() && w.is_empty(),
            "shape-only entity is silent: e={e:?} w={w:?}"
        );
    }

    #[test]
    fn inert_binding_fields_without_a_table_warn() {
        let s = SchemaTables::parse(BAS_SCHEMA);
        let (e, w) = validate_binding("site", &ent(None, None, Some("id"), None, None), &s);
        assert!(e.is_empty());
        assert!(w.iter().any(|m| m.contains("inert")), "{w:?}");
    }

    #[test]
    fn an_opaque_schema_warns_the_table_is_unverifiable_not_absent() {
        // Postgres pack: no readable DDL → every bound table is opaque, warned, never errored.
        let s = SchemaTables::parse("");
        let (e, w) = validate_binding(
            "site",
            &ent(None, Some("site"), Some("id"), None, Some("name")),
            &s,
        );
        assert!(e.is_empty());
        assert!(w.iter().any(|m| m.contains("opaque")), "{w:?}");
    }
}
