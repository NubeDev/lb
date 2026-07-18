//! The linter behind `pack.validate` — the dry-run gate a pack author runs in CI.
//!
//! ERRORS gate the apply (a duplicate object id, a dangling entity parent: the pack is
//! self-inconsistent and applying it would produce something the author did not describe).
//! WARNINGS print but never gate — the SQL dialect poison list is the canonical case: it is folklore
//! that will drift, and the real oracle is applying against the real node. Gating on a hardcoded
//! blocklist would refuse valid packs as the dialect support moves.

use std::collections::BTreeSet;

use crate::bundle::Pack;
use crate::plan::PlannedObject;

/// Substrings that commonly break the DataFusion∩SQLite intersection a federated source is read
/// through. A warning list, deliberately — see the module doc.
pub const SQL_POISON: &[&str] = &["datetime(", "with ", "strftime("];

/// One lint finding. `error: true` gates the apply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub error: bool,
    pub message: String,
}

/// Lint a resolved pack against its own plan.
pub fn validate(pack: &Pack, plan: &[PlannedObject]) -> Vec<Finding> {
    let mut out = Vec::new();

    // ERROR — a duplicate (kind, id) means two objects would write the same target, and the receipt
    // could not tell them apart.
    let mut seen = BTreeSet::new();
    for o in plan {
        let key = (o.kind.as_str(), o.id.as_str());
        if !seen.insert(key) {
            out.push(Finding {
                error: true,
                message: format!(
                    "duplicate {} id '{}' — object ids must be unique",
                    o.kind.as_str(),
                    o.id
                ),
            });
        }
    }

    // ERROR — a dangling entity parent makes the vocabulary tree unrenderable.
    for (name, ent) in &pack.manifest.entities {
        if let Some(parent) = &ent.parent {
            if !pack.manifest.entities.contains_key(parent) {
                out.push(Finding {
                    error: true,
                    message: format!(
                        "entity '{name}' names parent '{parent}', which is not declared"
                    ),
                });
            }
        }
    }

    // WARNING — dialect poison in the schema/seed SQL.
    for sql in [&pack.schema_sql, &pack.seed_sql].into_iter().flatten() {
        let lower = sql.to_lowercase();
        for needle in SQL_POISON {
            if lower.contains(needle) {
                out.push(Finding {
                    error: false,
                    message: format!(
                        "SQL contains '{needle}' — often unsupported in the DataFusion∩SQLite \
                         intersection a federated read goes through; verify against a real apply"
                    ),
                });
            }
        }
    }

    // WARNING — schema/seed declared for an engine the host cannot materialize: the source will
    // register, but the SQL will not run, and the author should know before they wonder why the
    // tables are empty.
    if let Some(ds) = &pack.manifest.datasource {
        if ds.engine != "sqlite" && (ds.schema.is_some() || ds.seed.is_some()) {
            out.push(Finding {
                error: false,
                message: format!(
                    "datasource '{}' declares schema/seed SQL but engine is '{}' — the source \
                     registers, the SQL does not run (materializing is sqlite-only)",
                    ds.name, ds.engine
                ),
            });
        }
    }

    out
}

/// True when any finding gates the apply.
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bundle::Bundle;
    use crate::plan::plan;
    use std::collections::BTreeMap;

    fn resolve(manifest: &str, files: &[(&str, &str)]) -> Pack {
        Bundle {
            manifest: manifest.into(),
            files: files
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
        .resolve()
        .unwrap()
    }

    #[test]
    fn a_clean_pack_has_no_findings() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\nrules: [rules/a.rhai]\n",
            &[("rules/a.rhai", "let x = 1;")],
        );
        let f = validate(&p, &plan(&p));
        assert!(f.is_empty(), "{f:?}");
    }

    #[test]
    fn a_dangling_entity_parent_is_an_error() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\nentities:\n  point:\n    label: Point\n    parent: equip\n",
            &[],
        );
        let f = validate(&p, &plan(&p));
        assert!(has_errors(&f), "{f:?}");
        assert!(f[0].message.contains("equip"), "{f:?}");
    }

    #[test]
    fn dialect_poison_warns_but_never_gates() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\ndatasource:\n  name: d\n  engine: sqlite\n  schema: s.sql\n",
            &[("s.sql", "SELECT datetime('now');")],
        );
        let f = validate(&p, &plan(&p));
        assert!(!has_errors(&f), "dialect poison must not gate: {f:?}");
        assert!(f.iter().any(|x| x.message.contains("datetime(")), "{f:?}");
    }

    #[test]
    fn a_duplicate_object_id_is_an_error() {
        // Two rule paths with the same stem collide on the rule id.
        let p = Pack {
            manifest: crate::manifest::Manifest::parse("pack: p\ntitle: P\nversion: 1\n").unwrap(),
            manifest_raw: String::new(),
            rules: vec![
                crate::bundle::LoadedRule {
                    id: "dup".into(),
                    name: "a".into(),
                    body: "1".into(),
                },
                crate::bundle::LoadedRule {
                    id: "dup".into(),
                    name: "b".into(),
                    body: "2".into(),
                },
            ],
            dashboards: vec![],
            schema_sql: None,
            seed_sql: None,
            agent_context: None,
        };
        let f = validate(&p, &plan(&p));
        assert!(has_errors(&f), "{f:?}");
    }

    #[test]
    fn the_plan_orders_datasource_first_and_agent_last() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\n\
             datasource:\n  name: d\n  engine: sqlite\n\
             rules: [rules/a.rhai]\n\
             channels:\n  - name: c\n\
             agent:\n  context: ctx.md\n",
            &[("rules/a.rhai", "let x = 1;"), ("ctx.md", "# context")],
        );
        let pl = plan(&p);
        let kinds: Vec<_> = pl.iter().map(|o| o.kind.as_str()).collect();
        assert_eq!(kinds, vec!["datasource", "rule", "channel", "agent"]);
        // Unused import guard for BTreeMap in this module's test helper path.
        let _: BTreeMap<String, String> = BTreeMap::new();
    }
}
