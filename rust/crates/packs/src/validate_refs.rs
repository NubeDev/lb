//! The `refs:` lint (`entity-source-refs-scope.md` §Validation) — shape, never data.
//!
//! Every check here is readable from the manifest ALONE, with no schema, no workspace and no node —
//! which is exactly why each one ERRORS rather than warns: they are self-inconsistencies in the
//! artifact the author is looking at, the class of the dangling-entity-parent error, not the class of
//! the opaque-schema warning [`crate::binding`] deals in.
//!
//! The line this file must not cross: **whether `source` names a datasource that exists is a
//! workspace fact, not a pack fact.** Datasources are registered per workspace and resolved late (the
//! same late binding every saved `federation.query` cell has), so a ref to an unregistered source is
//! "this pack offers no such source *here*" — a read-time nothing, never a validate gate. Gating on
//! it would refuse a valid pack on every node but the one it was authored against.

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::Entity;
use crate::validate::Finding;

/// Lint every entity's `refs:` block, plus the `charts.source` unlock that depends on it.
pub fn lint(entities: &BTreeMap<String, Entity>) -> Vec<Finding> {
    let mut out = Vec::new();
    for (name, ent) in entities {
        lint_entity(name, ent, &mut out);
    }
    out
}

fn lint_entity(name: &str, ent: &Entity, out: &mut Vec<Finding>) {
    let mut err = |message: String| {
        out.push(Finding {
            error: true,
            message,
        })
    };

    // A ref declares "this entity's PK is also the key over there" — so an entity with no pk (a
    // shape-only vocabulary entity, or a table binding without one) has nothing to be the key.
    if !ent.refs.is_empty() && (ent.table.is_none() || ent.pk.is_none()) {
        err(format!(
            "entity '{name}' declares refs but is not bound (needs both `table` and `pk`) — a ref \
             says this entity's pk is also the key in a datasource, so there must be a pk to mean"
        ));
    }

    // Bare identifiers only, the `geo:` derivation discipline: a downstream builder INTERPOLATES
    // `table`/`fk` into SQL it derives, so anything needing quoting is refused at the door rather
    // than quoted for the author (refuse, don't quote).
    let mut seen: BTreeSet<(&str, &str)> = BTreeSet::new();
    for r in &ent.refs {
        if r.source.trim().is_empty() {
            err(format!(
                "entity '{name}' declares a ref with an empty `source` — a ref addresses a \
                 datasource by name"
            ));
        }
        for (field, value) in [("table", Some(&r.table)), ("fk", r.fk.as_ref())] {
            if let Some(value) = value {
                if !is_bare_identifier(value) {
                    err(format!(
                        "entity '{name}' ref to source '{}' has {field} '{value}', which is not a \
                         bare identifier — a downstream read derives SQL from it, so it must need no \
                         quoting",
                        r.source
                    ));
                }
            }
        }
        // A duplicate {source, table} is either a copy-paste or two refs meaning different things
        // under one address — both are unresolvable downstream (which ref did the chart mean?).
        if !seen.insert((r.source.as_str(), r.table.as_str())) {
            err(format!(
                "entity '{name}' declares the ref '{}'/'{}' twice — {{source, table}} addresses one \
                 twin, and a duplicate is ambiguous downstream",
                r.source, r.table
            ));
        }
    }

    lint_chart_sources(name, ent, out);
}

/// The `charts.source` unlock: on a **store** entity a chart recipe may name a datasource — but only
/// one the entity has declared a ref to. That dangling-in-manifest case is the author's bug and is
/// readable with nothing but the manifest, so it gates.
///
/// Deliberately scoped to `backend: store` ONLY. A `datasource` entity's chart `source` is an ordinary
/// override that predates refs, and an entity with NO explicit backend is routed downstream by the
/// pack's `datasource.engine` — gating either would break packs that validate clean today, for no
/// safety gained.
fn lint_chart_sources(name: &str, ent: &Entity, out: &mut Vec<Finding>) {
    if ent.backend != Some(crate::manifest::Backend::Store) {
        return;
    }
    for chart in &ent.charts {
        let Some(source) = chart.source.as_deref() else {
            continue;
        };
        if !ent.refs.iter().any(|r| r.source == source) {
            out.push(Finding {
                error: true,
                message: format!(
                    "entity '{name}' chart '{}' reads source '{source}', but the entity is \
                     store-backed and declares no ref to it — add it to `refs:` (a store entity may \
                     chart a datasource only through a declared twin)",
                    chart.key
                ),
            });
        }
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*` — what a derived read can interpolate unquoted in every dialect the
/// federation layer spans.
fn is_bare_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    fn findings(entities_yaml: &str) -> Vec<Finding> {
        let m = Manifest::parse(&format!(
            "pack: p\ntitle: P\nversion: 1\nentities:\n{entities_yaml}"
        ))
        .unwrap();
        lint(&m.entities)
    }

    fn messages(f: &[Finding]) -> String {
        f.iter()
            .map(|x| x.message.as_str())
            .collect::<Vec<_>>()
            .join(" | ")
    }

    const BOUND_SITE: &str = "  site:\n    label: Site\n    table: ems_site\n    pk: id\n    \
                              backend: store\n";

    #[test]
    fn a_well_formed_ref_is_clean() {
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: demo-buildings, table: site, fk: id }}\n"
        ));
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn an_unregistered_source_never_gates() {
        // The load-bearing non-check: `source` resolves per WORKSPACE at read time. Nothing about a
        // name core has never heard of is a pack defect.
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: not-registered-anywhere, table: site }}\n"
        ));
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn a_ref_on_an_unbound_entity_errors() {
        let f =
            findings("  site:\n    label: Site\n    refs:\n      - { source: d, table: site }\n");
        assert!(
            f.iter().any(|x| x.error && x.message.contains("not bound")),
            "{}",
            messages(&f)
        );
        // …and a table binding with no pk is the same defect: there is no key to mean.
        let f = findings(
            "  site:\n    label: Site\n    table: ems_site\n    refs:\n      - { source: d, table: site }\n",
        );
        assert!(f.iter().any(|x| x.error), "{}", messages(&f));
    }

    #[test]
    fn a_non_identifier_table_or_fk_errors() {
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: d, table: \"my table\" }}\n"
        ));
        assert!(
            f.iter()
                .any(|x| x.error && x.message.contains("bare identifier")),
            "{}",
            messages(&f)
        );

        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: d, table: site, fk: \"id; DROP TABLE x\" }}\n"
        ));
        assert!(
            f.iter().any(|x| x.error && x.message.contains("fk")),
            "{}",
            messages(&f)
        );
    }

    #[test]
    fn a_duplicate_source_table_pair_errors() {
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: d, table: site }}\n      \
             - {{ source: d, table: site, fk: other_id }}\n"
        ));
        assert!(
            f.iter().any(|x| x.error && x.message.contains("twice")),
            "{}",
            messages(&f)
        );

        // Two refs to DIFFERENT tables of the same source are legitimate — a site has a `site` row
        // and a `point_reading` history in one database.
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: d, table: site }}\n      \
             - {{ source: d, table: point_reading, fk: site_id }}\n"
        ));
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn a_store_entity_may_chart_a_declared_ref() {
        // The unlock (§4): legal precisely because the ref declares the twin.
        let f = findings(&format!(
            "{BOUND_SITE}    refs:\n      - {{ source: demo-buildings, table: site }}\n    \
             charts:\n      - {{ key: demand, label: Demand, source: demo-buildings, \
             table: point_reading, columns: {{ time: ts, value: val, entity: site_id }} }}\n"
        ));
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn a_store_entity_charting_an_undeclared_source_errors() {
        let f = findings(&format!(
            "{BOUND_SITE}    charts:\n      - {{ key: demand, label: Demand, source: demo-buildings }}\n"
        ));
        assert!(
            f.iter()
                .any(|x| x.error && x.message.contains("declares no ref")),
            "{}",
            messages(&f)
        );
    }

    #[test]
    fn a_store_entity_chart_with_no_source_is_untouched() {
        // The pre-refs shape: routed by `backend`, reads the store. Nothing to check.
        let f = findings(&format!(
            "{BOUND_SITE}    charts:\n      - {{ key: energy, label: Energy, table: ems_reading }}\n"
        ));
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn a_datasource_entity_chart_source_is_not_gated() {
        // Pre-existing behaviour: on a datasource entity `source` is an ordinary override, refs or
        // not. Gating it would break packs that validate clean today.
        let f = findings(
            "  reading:\n    label: Reading\n    table: point_reading\n    pk: id\n    \
             backend: datasource\n    charts:\n      - { key: d, label: D, source: anything }\n",
        );
        assert!(f.is_empty(), "{}", messages(&f));

        // …and so is an entity with NO explicit backend (routed by `datasource.engine` downstream).
        let f = findings(
            "  reading:\n    label: Reading\n    table: point_reading\n    pk: id\n    \
             charts:\n      - { key: d, label: D, source: anything }\n",
        );
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn an_entity_with_no_refs_is_silent() {
        let f = findings("  site: { label: Site }\n");
        assert!(f.is_empty(), "{}", messages(&f));
    }

    #[test]
    fn bare_identifier_accepts_and_refuses_the_right_shapes() {
        assert!(is_bare_identifier("site"));
        assert!(is_bare_identifier("_site_id2"));
        assert!(!is_bare_identifier(""));
        assert!(!is_bare_identifier("2fast"));
        assert!(!is_bare_identifier("main.site")); // schema-qualified needs a different ask
        assert!(!is_bare_identifier("\"site\""));
    }
}
