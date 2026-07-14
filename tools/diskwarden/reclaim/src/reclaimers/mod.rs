//! The reclaimer registry — the only place any reclaimer is named.
//!
//! Everything downstream (scan, clean, the tray, the UI) iterates `all()` and reads
//! `Reclaimer::id()` as opaque data. Adding a cleaner is: one new file here, one
//! line in `all()`. Nothing else changes.

pub mod cargo_target;

use crate::reclaimer::Reclaimer;

/// Every reclaimer this build knows about.
pub fn all() -> Vec<Box<dyn Reclaimer>> {
    vec![Box::new(cargo_target::CargoTarget)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn ids_are_unique() {
        let ids: Vec<_> = all().iter().map(|r| r.id()).collect();
        let unique: BTreeSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "duplicate reclaimer id: {ids:?}");
    }

    /// Ids are policy keys and state-file keys — they must stay kebab-case and stable.
    #[test]
    fn ids_are_kebab_case_and_nonempty() {
        for r in all() {
            let id = r.id();
            assert!(!id.is_empty());
            assert!(
                id.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "id {id:?} must be kebab-case"
            );
            assert!(!r.describe().is_empty(), "{id} needs a description");
        }
    }
}
