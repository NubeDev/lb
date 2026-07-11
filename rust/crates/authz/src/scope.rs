//! [`Scope`] — the **entity-scoped grant** selector: a narrowing of a grant's reach to a subset
//! of a table's rows *within* a workspace (entity-scoped-grants scope). The core never interprets
//! the table/id values — they are opaque data to the platform (rule 10); the extension that
//! creates the scoped grant owns *what table means*, the core owns *what scope means*.
//!
//! A grant with `Scope::All` (the default) behaves exactly like today's grants — zero migration.
//! A grant with `Scope::Ids { table, ids }` narrows the cap to only those ids in that table.
//! The resolver unions all scoped grants for the same cap: a principal holding
//! `mcp:care.log.list:call` with `Ids{child, [leo]}` and another with `Ids{child, [mia]}`
//! resolves to `Ids{child, [leo, mia]}` — one union, one cached read.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// The resource selector on a grant. `All` = today's behaviour (every row); `Ids` narrows to a
/// named-table subset. Default `All` so old grant records (no `scope` field) deserialize to
/// today's behaviour with zero migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Scope {
    /// The grant reaches every record in the workspace (today's behaviour). The default.
    #[default]
    All,
    /// The grant reaches only the listed record ids in `table`. `table` and `ids` are opaque
    /// data to the core (rule 10) — the extension owns their meaning.
    Ids { table: String, ids: Vec<String> },
}

impl Scope {
    /// A deterministic key fragment for the grant record id. `All` → empty (so the existing
    /// `grant_id(subject, cap)` key is unchanged for backward compat). `Ids` → `table:sorted_ids`.
    pub(crate) fn key(&self) -> String {
        match self {
            Scope::All => String::new(),
            Scope::Ids { table, ids } => {
                let mut sorted = ids.clone();
                sorted.sort();
                sorted.dedup();
                format!("{table}:{}", sorted.join(","))
            }
        }
    }

    /// Does this scope allow access to `id` in `table`? `All` → yes for any table/id. `Ids` →
    /// only if `table` matches and `ids` contains `id`.
    pub fn contains(&self, table: &str, id: &str) -> bool {
        match self {
            Scope::All => true,
            Scope::Ids { table: t, ids } => t == table && ids.iter().any(|i| i == id),
        }
    }

    /// Union two scopes for the same cap. `All` absorbs `Ids` (any `All` grant wins — the cap is
    /// fully reachable). Two `Ids` for the same table merge their id sets. `Ids` for different
    /// tables is an edge case that shouldn't arise (one cap, one table per domain); if it does,
    /// the union is `All` (conservative — widen to safe, not to a mixed-table selector).
    pub fn union(&self, other: &Scope) -> Scope {
        match (self, other) {
            (Scope::All, _) | (_, Scope::All) => Scope::All,
            (Scope::Ids { table: t1, ids: i1 }, Scope::Ids { table: t2, ids: i2 }) => {
                if t1 != t2 {
                    return Scope::All;
                }
                let mut merged: BTreeSet<String> = i1.iter().cloned().collect();
                merged.extend(i2.iter().cloned());
                Scope::Ids {
                    table: t1.clone(),
                    ids: merged.into_iter().collect(),
                }
            }
        }
    }

    /// Convert to a [`ScopeFilter`] for `table`. `All` → `ScopeFilter::All`. `Ids` for a different
    /// table → `ScopeFilter::Ids(vec![])` (this cap is scoped to a different table; none of
    /// `table`'s rows are reachable). `Ids` for the same table → `ScopeFilter::Ids(ids)`.
    pub fn filter_for(&self, table: &str) -> ScopeFilter {
        match self {
            Scope::All => ScopeFilter::All,
            Scope::Ids { table: t, ids } => {
                if t == table {
                    ScopeFilter::Ids(ids.clone())
                } else {
                    ScopeFilter::Ids(vec![])
                }
            }
        }
    }

    /// True if this is `Scope::All` (used for `skip_serializing_if` to keep old records clean).
    pub fn is_all(&self) -> bool {
        matches!(self, Scope::All)
    }
}

/// The query-side filter result: either `All` (every row reachable) or `Ids` (only these rows).
/// Returned by [`scope_filter`](crate::check_scoped::scope_filter) so a `list` verb pushes the
/// ids into one indexed query instead of post-filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ScopeFilter {
    /// The cap is fully reachable — no row-level filter needed.
    All,
    /// Only these record ids are reachable (empty = the cap is held but scoped to zero rows in
    /// this table — degrade to empty, not error).
    Ids(Vec<String>),
}

impl ScopeFilter {
    /// Convenience: the ids if `Ids`, or `None` if `All`.
    pub fn ids(&self) -> Option<&[String]> {
        match self {
            ScopeFilter::All => None,
            ScopeFilter::Ids(ids) => Some(ids),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_contains_any() {
        assert!(Scope::All.contains("child", "leo"));
        assert!(Scope::All.contains("site", "north"));
    }

    #[test]
    fn ids_contains_only_listed() {
        let s = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into(), "mia".into()],
        };
        assert!(s.contains("child", "leo"));
        assert!(s.contains("child", "mia"));
        assert!(!s.contains("child", "sam"));
        assert!(!s.contains("site", "leo"));
    }

    #[test]
    fn union_all_absorbs_ids() {
        let ids = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        };
        assert_eq!(Scope::All.union(&ids), Scope::All);
        assert_eq!(ids.union(&Scope::All), Scope::All);
    }

    #[test]
    fn union_ids_same_table_merges() {
        let a = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        };
        let b = Scope::Ids {
            table: "child".into(),
            ids: vec!["mia".into(), "leo".into()],
        };
        let u = a.union(&b);
        match u {
            Scope::Ids { table, ids } => {
                assert_eq!(table, "child");
                assert!(ids.contains(&"leo".to_string()));
                assert!(ids.contains(&"mia".to_string()));
                assert_eq!(ids.len(), 2);
            }
            _ => panic!("expected Ids"),
        }
    }

    #[test]
    fn union_ids_different_table_widens_to_all() {
        let a = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        };
        let b = Scope::Ids {
            table: "site".into(),
            ids: vec!["north".into()],
        };
        assert_eq!(a.union(&b), Scope::All);
    }

    #[test]
    fn filter_for_all_returns_all() {
        assert_eq!(Scope::All.filter_for("child"), ScopeFilter::All);
    }

    #[test]
    fn filter_for_ids_same_table_returns_ids() {
        let s = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into(), "mia".into()],
        };
        assert_eq!(
            s.filter_for("child"),
            ScopeFilter::Ids(vec!["leo".into(), "mia".into()])
        );
    }

    #[test]
    fn filter_for_ids_different_table_returns_empty() {
        let s = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        };
        assert_eq!(s.filter_for("site"), ScopeFilter::Ids(vec![]));
    }

    #[test]
    fn key_for_all_is_empty() {
        assert_eq!(Scope::All.key(), "");
    }

    #[test]
    fn key_for_ids_is_deterministic() {
        let a = Scope::Ids {
            table: "child".into(),
            ids: vec!["mia".into(), "leo".into()],
        };
        let b = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into(), "mia".into()],
        };
        assert_eq!(a.key(), b.key());
        assert_eq!(a.key(), "child:leo,mia");
    }

    #[test]
    fn serde_all_round_trips() {
        let s = Scope::All;
        let json = serde_json::to_string(&s).unwrap();
        let back: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn serde_ids_round_trips() {
        let s = Scope::Ids {
            table: "child".into(),
            ids: vec!["leo".into()],
        };
        let json = serde_json::to_string(&s).unwrap();
        let back: Scope = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }

    #[test]
    fn missing_scope_field_defaults_to_all() {
        let json = r#"{"subject":"user:ada","cap":"mcp:foo:call"}"#;
        let grant: super::super::grant::Grant = serde_json::from_str(json).unwrap();
        assert_eq!(grant.scope, Scope::All);
    }
}
