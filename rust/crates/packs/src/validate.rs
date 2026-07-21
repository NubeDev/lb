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

/// Strip SQL comments so the poison scan sees only executable text.
///
/// Without this, a pack that WARNS ITS OWN AUTHOR off `datetime()` in a comment is flagged for the
/// very thing the comment tells them to avoid — the linter reading prose as SQL. The scan is a
/// substring match by design (see the module doc), so the only honest fix is to narrow what it sees.
///
/// Handles `--` to end-of-line and `/* … */` (non-nesting, as SQLite). String literals are tracked
/// so a comment marker INSIDE a literal survives: `'-- not a comment'` is seed data, not a comment,
/// and dropping it there would corrupt the very text the scan then reads. Escaped quotes in SQL are
/// doubled (`''`), which falls out of the toggle naturally: the pair flips the flag off and back on.
fn strip_sql_comments(sql: &str) -> String {
    let b = sql.as_bytes();
    let mut out = String::with_capacity(sql.len());
    let mut i = 0;
    // Which literal we are inside, if any — a comment marker within one is data, not a comment.
    let mut quote: Option<u8> = None;

    while i < b.len() {
        let c = b[i];
        match quote {
            Some(q) => {
                out.push(c as char);
                if c == q {
                    quote = None;
                }
                i += 1;
            }
            None => {
                if c == b'\'' || c == b'"' {
                    quote = Some(c);
                    out.push(c as char);
                    i += 1;
                } else if c == b'-' && b.get(i + 1) == Some(&b'-') {
                    // `--` to end of line. Keep the newline so line structure (and any `with ` at a
                    // following line's start) is preserved for the scan.
                    while i < b.len() && b[i] != b'\n' {
                        i += 1;
                    }
                } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
                    i += 2;
                    while i < b.len() && !(b[i] == b'*' && b.get(i + 1) == Some(&b'/')) {
                        i += 1;
                    }
                    // Skip the closing `*/`; an unterminated block runs to EOF, as SQLite treats it.
                    i = (i + 2).min(b.len());
                    // A block comment separates tokens — collapse it to a space so `a/**/b` does not
                    // become the single token `ab`.
                    out.push(' ');
                } else {
                    // Multi-byte UTF-8 is copied byte-wise; no marker byte appears inside a
                    // continuation byte, so byte-level scanning is safe here.
                    out.push(c as char);
                    i += 1;
                }
            }
        }
    }
    out
}

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

    // ENTITY→TABLE BINDING lint (pack-entity-binding-scope.md). Errors are manifest-only structural
    // inconsistencies (parent_fk with no parent); everything schema-referential is a WARNING, since a
    // pack's schema can be opaque (postgres) and the real oracle is the apply — the dialect precedent.
    let schema = crate::binding::SchemaTables::parse(pack.schema_sql.as_deref().unwrap_or(""));
    for (name, ent) in &pack.manifest.entities {
        let (errs, warns) = crate::binding::validate_binding(name, ent, &schema);
        for message in errs {
            out.push(Finding {
                error: true,
                message,
            });
        }
        for message in warns {
            out.push(Finding {
                error: false,
                message,
            });
        }
    }

    // WARNING — dialect poison in the schema/seed SQL. Comments are stripped first: the scan is a
    // substring match, so without this a comment WARNING the author off `datetime()` trips the very
    // warning it is telling them to avoid.
    for sql in [&pack.schema_sql, &pack.seed_sql].into_iter().flatten() {
        let lower = strip_sql_comments(sql).to_lowercase();
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

    /// NubeDev/lb#80: the scan is a substring match over raw text, so a comment that WARNS ITS AUTHOR
    /// off `datetime()` trips the very warning it is telling them to avoid — the linter reading prose
    /// as SQL.
    ///
    /// Note the issue as filed says "a pack comment flags the pack", which does not reproduce on the
    /// packs that exist today: their dialect warnings live in `.rhai` rule comments, and this scan
    /// reads only `schema_sql`/`seed_sql` (`SQL_POISON` has exactly one consumer). The fault is real
    /// but currently LATENT — the first `-- avoid datetime()` in a pack's schema.sql triggers it.
    /// Fixed here rather than after an author hits it.
    #[test]
    fn poison_inside_a_comment_does_not_warn() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\ndatasource:\n  name: d\n  engine: sqlite\n  schema: s.sql\n",
            &[(
                "s.sql",
                "-- Do NOT use datetime() here; it breaks the federated read.\n\
                 /* strftime( is out too, and so is\n   a with  clause. */\n\
                 CREATE TABLE t (id TEXT PRIMARY KEY);",
            )],
        );
        let f = validate(&p, &plan(&p));
        assert!(
            f.is_empty(),
            "comments must not trip the poison scan: {f:?}"
        );
    }

    /// The other half: stripping comments must not blind the scan to REAL poison next to them.
    #[test]
    fn poison_in_code_still_warns_alongside_comments() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\ndatasource:\n  name: d\n  engine: sqlite\n  schema: s.sql\n",
            &[(
                "s.sql",
                "-- a harmless comment mentioning nothing\nSELECT datetime('now'); -- trailing\n",
            )],
        );
        let f = validate(&p, &plan(&p));
        assert!(
            f.iter().any(|x| x.message.contains("datetime(")),
            "real poison must still warn: {f:?}"
        );
    }

    /// A comment marker inside a string literal is DATA. Dropping it there would corrupt the text the
    /// scan then reads — and could hide real poison behind a `--` that was never a comment.
    #[test]
    fn a_comment_marker_inside_a_literal_is_data() {
        let stripped = strip_sql_comments("INSERT INTO t VALUES ('-- datetime( stays');");
        assert!(
            stripped.contains("datetime("),
            "a literal's contents must survive: {stripped}"
        );
        // And the scan agrees: this pack warns, because the poison is really there.
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\ndatasource:\n  name: d\n  engine: sqlite\n  seed: s.sql\n",
            &[("s.sql", "INSERT INTO t VALUES ('-- datetime( stays');")],
        );
        let f = validate(&p, &plan(&p));
        assert!(f.iter().any(|x| x.message.contains("datetime(")), "{f:?}");
    }

    #[test]
    fn comment_stripping_handles_the_awkward_shapes() {
        // Doubled quotes are SQL's escape — the pair toggles the flag off and back on.
        assert!(strip_sql_comments("SELECT 'it''s here -- x' , 1;").contains("it''s here -- x"));
        // A block comment separates tokens rather than joining them.
        assert_eq!(strip_sql_comments("a/**/b").trim(), "a b");
        // An unterminated block runs to EOF and does not panic.
        assert_eq!(
            strip_sql_comments("SELECT 1 /* never closed").trim(),
            "SELECT 1"
        );
        // A `--` comment keeps its newline, so the next line's leading token is still scannable.
        assert!(strip_sql_comments("-- c\nwith x as (select 1)").starts_with('\n'));
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
    fn the_plan_orders_datasource_first_and_sidebar_last() {
        let p = resolve(
            "pack: p\ntitle: P\nversion: 1\n\
             datasource:\n  name: d\n  engine: sqlite\n\
             rules: [rules/a.rhai]\n\
             channels:\n  - name: c\n\
             agent:\n  context: ctx.md\n\
             sidebar:\n  hidden: [channels]\n",
            &[("rules/a.rhai", "let x = 1;"), ("ctx.md", "# context")],
        );
        let pl = plan(&p);
        let kinds: Vec<_> = pl.iter().map(|o| o.kind.as_str()).collect();
        assert_eq!(
            kinds,
            vec!["datasource", "rule", "channel", "agent", "sidebar"]
        );
        // Unused import guard for BTreeMap in this module's test helper path.
        let _: BTreeMap<String, String> = BTreeMap::new();
    }

    #[test]
    fn a_changed_hidden_set_is_drift_at_the_same_version() {
        // Same pack version, a different hidden-set → a different content checksum, so the refusal
        // matrix re-applies (full-set LWW clobbers) rather than treating it as an idempotent no-op.
        use crate::plan::content_checksum;
        let a = resolve(
            "pack: p\ntitle: P\nversion: 1\nsidebar:\n  hidden: [channels]\n",
            &[],
        );
        let b = resolve(
            "pack: p\ntitle: P\nversion: 1\nsidebar:\n  hidden: [channels, datasources]\n",
            &[],
        );
        assert_ne!(content_checksum(&a), content_checksum(&b));
    }
}
