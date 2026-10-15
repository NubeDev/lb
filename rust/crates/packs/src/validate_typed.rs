//! The `typed_value:` lint (`entity-typed-value-scope.md` §Goals 3) — shape, never data.
//!
//! Every check here is readable from the manifest ALONE, so each one ERRORS, the class of the
//! `refs:` lint ([`crate::validate_refs`]). Whether the named columns exist in the bound table is a
//! schema question and lives with the rest of the binding's column checks in [`crate::binding`], as a
//! warning (an opaque schema cannot prove absence).
//!
//! The line this file must not cross: it never looks at a row. A `type` word downstream does not
//! know, a `value` that does not match its schema — those are data, and data is downstream's.

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::Entity;
use crate::validate::Finding;

/// Lint every entity's `typed_value:` block.
pub fn lint(entities: &BTreeMap<String, Entity>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, ent) in entities {
        let Some(typed) = &ent.typed_value else {
            continue;
        };
        let mut err = |message: String| {
            out.push(Finding {
                error: true,
                message,
            })
        };
        if ent.table.is_none() {
            err(format!(
                "entity '{name}' declares typed_value but binds no table — it names columns of the \
                 bound table, so there must be one"
            ));
        }
        let mut seen = BTreeSet::new();
        for (field, column) in typed.columns() {
            // Bare identifiers only, the `refs:`/`geo:` discipline: a downstream read derives SQL from
            // these names, so anything needing quoting is refused rather than quoted.
            if !crate::validate_refs::is_bare_identifier(column) {
                err(format!(
                    "entity '{name}' typed_value {field} '{column}' is not a bare identifier — a \
                     downstream read derives SQL from it, so it must need no quoting"
                ));
            }
            if !seen.insert(column) {
                err(format!(
                    "entity '{name}' typed_value names column '{column}' twice — type, value and \
                     schema are three different things"
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    fn findings(typed: &str, table: bool) -> Vec<String> {
        let yaml = format!(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  setting:\n    label: Setting\n{}    typed_value:\n{typed}",
            if table { "    table: setting\n    pk: id\n" } else { "" }
        );
        let m = Manifest::parse(&yaml).expect("parses");
        lint(&m.entities).into_iter().map(|f| f.message).collect()
    }

    #[test]
    fn a_clean_block_passes_silently() {
        assert!(findings(
            "      type: type\n      value: value\n      schema: schema\n",
            true
        )
        .is_empty());
        assert!(findings("      type: type\n      value: value\n", true).is_empty());
    }

    #[test]
    fn no_table_is_an_error() {
        let f = findings("      type: type\n      value: value\n", false);
        assert!(f.iter().any(|m| m.contains("binds no table")), "{f:?}");
    }

    #[test]
    fn one_column_named_twice_is_an_error() {
        let f = findings("      type: value\n      value: value\n", true);
        assert!(f.iter().any(|m| m.contains("twice")), "{f:?}");
    }

    #[test]
    fn a_name_needing_quotes_is_an_error() {
        let f = findings("      type: type\n      value: \"the value\"\n", true);
        assert!(f.iter().any(|m| m.contains("bare identifier")), "{f:?}");
    }
}
