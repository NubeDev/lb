//! The rule-cage function catalog — the **single source of truth** for what a rule body can call.
//! `rules.help` returns it verbatim; the skill doc + UI autocomplete read it. Hand-curated (not
//! rhai's `gen_fn_signatures()`) so each entry carries a human description + family — the whole
//! point of the introspection surface.
//!
//! [`CATALOG`] chains the rows: the pre-stdlib [`core`] families (whose registrations are spread
//! across `verbs/mod.rs` + `verbs/*.rs`) plus each data-stdlib family's own `CATALOG` const, which
//! lives BESIDE its `register_fn` sites so a new verb and its row land in one file.
//!
//! **Maintenance rule:** every new `engine.register_fn(...)` MUST add a row in the same change —
//! in that family's file. Overload arities share ONE row (`names_are_unique`), with a combined
//! `sig1 | sig2` signature.

mod core;

use core::CORE;

/// One function in the catalog. `name` is the rhai call form (bare for free fns like `source`,
/// `family.method` for handle methods like `ai.ask`). Overload arities SHARE one row (`name` is
/// unique — the `names_are_unique` test) with a combined `sig1 | sig2` signature.
#[derive(Debug, Clone, Copy)]
pub struct FnEntry {
    /// The rhai call name. Bare (`source`) for free functions, `handle.method` (`ai.ask`) for the
    /// four scope handles, `value.method` (`g.filter`, `c.max`) for the grid/col chainable surface.
    pub name: &'static str,
    /// The verb family — matches the `verbs/*.rs` module name (`data`, `timeseries`, `emit`, `ai`,
    /// `messaging` for inbox+outbox+channel, `grid` for the lazy Grid methods, `frame` for polars).
    pub family: &'static str,
    /// The rhai signature, argument-first (`source(name: String) -> Grid`).
    pub signature: &'static str,
    /// One-line human description. This is the value the catalog adds over rhai's raw signatures.
    pub description: &'static str,
}

/// The full catalog: the [`core`] families + each data-stdlib family's own rows. Append-only
/// within a family — never reorder existing rows.
pub static CATALOG: std::sync::LazyLock<Vec<FnEntry>> = std::sync::LazyLock::new(|| {
    let mut all: Vec<FnEntry> = CORE.to_vec();
    all.extend_from_slice(crate::verbs::time::CATALOG);
    all.extend_from_slice(crate::verbs::duration::CATALOG);
    all.extend_from_slice(crate::verbs::json::CATALOG);
    all.extend_from_slice(crate::verbs::stats::CATALOG);
    all.extend_from_slice(crate::verbs::window::CATALOG);
    all.extend_from_slice(crate::verbs::mathx::CATALOG);
    all.extend_from_slice(crate::verbs::job::CATALOG);
    #[cfg(feature = "frames")]
    all.extend_from_slice(crate::verbs::frame::CATALOG);
    all
});

#[cfg(test)]
mod tests {
    //! Catalog integrity — the descriptions can't be auto-validated, but the structure can. These
    //! catch a missing/typo'd entry the day a `register_fn` is added without a row here.

    use super::*;
    use std::collections::HashSet;

    #[test]
    fn names_are_unique() {
        let mut seen = HashSet::new();
        for e in CATALOG.iter() {
            assert!(seen.insert(e.name), "duplicate catalog name: {}", e.name);
        }
    }

    #[test]
    fn names_are_valid_rhai_paths() {
        // Bare identifier or handle/name.path — reject spaces, dots at the edges, empty parts.
        for e in CATALOG.iter() {
            let name = e.name;
            assert!(!name.is_empty(), "empty name");
            assert!(
                !name.starts_with('.') && !name.ends_with('.'),
                "edge dot: {name}"
            );
            for part in name.split('.') {
                assert!(!part.is_empty(), "empty path part in {name}");
                let first = part.chars().next().unwrap();
                assert!(
                    first.is_ascii_alphabetic() || first == '_',
                    "name {name:?} part {part:?} must start with a letter/_"
                );
                assert!(
                    part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                    "name {name:?} part {part:?} has non-identifier chars"
                );
            }
        }
    }

    #[test]
    fn every_entry_has_nonempty_fields() {
        for e in CATALOG.iter() {
            assert!(!e.family.is_empty(), "empty family for {}", e.name);
            assert!(!e.signature.is_empty(), "empty signature for {}", e.name);
            assert!(
                !e.description.is_empty(),
                "empty description for {}",
                e.name
            );
            assert!(
                e.description.ends_with('.'),
                "description for {} should end with '.' (sentence)",
                e.name
            );
        }
    }

    #[test]
    fn families_are_the_known_set() {
        // A new family here is a deliberate act (catches a typo like "messagingg").
        let known: HashSet<&str> = [
            "data",
            "grid",
            "timeseries",
            "chart",
            "emit",
            "ai",
            "messaging",
            "insight",
            "time",
            "json",
            "stats",
            "mathx",
            "job",
            "frame",
        ]
        .into_iter()
        .collect();
        for e in CATALOG.iter() {
            assert!(
                known.contains(e.family),
                "unknown family {:?} on {}",
                e.family,
                e.name
            );
        }
    }

    #[test]
    fn catalog_has_entries_from_every_verb_module() {
        // A floor — if any family disappears entirely, a verb module's registrations lost their
        // catalog rows. (The precise register_fn↔catalog count is enforced manually at review; this
        // catches a wholesale drop.)
        let families: HashSet<&str> = CATALOG.iter().map(|e| e.family).collect();
        for required in [
            "data",
            "grid",
            "timeseries",
            "chart",
            "emit",
            "ai",
            "messaging",
            "insight",
            "time",
            "json",
            "stats",
            "mathx",
            "job",
        ] {
            assert!(
                families.contains(required),
                "family {required} has no entries"
            );
        }
    }
}
