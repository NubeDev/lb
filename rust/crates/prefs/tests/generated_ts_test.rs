//! The closed axis vocabulary must stay self-consistent: `ALL` is what the `gen-prefs-ts`
//! generator and every exhaustive test enumerate, so a variant added without updating `ALL`
//! silently drops out of the generated client constants and the unit picker.
//!
//! This file once also drift-tested the generated client twin
//! (`ui/src/lib/prefs/dimensions.generated.ts`) for byte-identity against the generator. That
//! guard died with the in-tree client (`678503f` "deleted the ui") — lb is a library now and the
//! consuming client lives out of tree, so there is no checked-in file to be identical *to*.
//! Re-pointing it at a regenerated file would only assert the generator matches itself. The
//! generator (`cargo run -p lb-prefs --bin gen-prefs-ts`) remains the way to emit the twin; a
//! consumer that vendors it owns its own drift test.

use lb_prefs::axis::{Dimension, Unit, UnitSystem};

#[test]
fn dimension_and_unit_counts_match_declared_all() {
    // Guard the ALL arrays against an enum variant added without updating ALL (the generator + every
    // exhaustive test reads ALL).
    assert_eq!(Dimension::ALL.len(), 18);
    assert_eq!(Unit::ALL.len(), 58);
    for u in Unit::ALL {
        assert_eq!(
            Unit::parse(u.as_str()),
            Some(u),
            "every unit token round-trips through parse"
        );
    }
}

/// Every dimension must be reachable from its own units, and its canonical unit must belong to it.
/// This is what stops a new dimension being added with a canonical unit borrowed from another one
/// (the BAS slice added ten dimensions at once — a copy-paste there would be silent).
#[test]
fn every_dimension_is_self_consistent() {
    for d in Dimension::ALL {
        assert_eq!(
            d.canonical_unit().dimension(),
            d,
            "{}'s canonical unit must belong to it",
            d.as_str()
        );
        assert!(
            Unit::ALL.iter().any(|u| u.dimension() == d),
            "{} has no units",
            d.as_str()
        );
        // The display default a system picks must also belong to the dimension.
        for sys in UnitSystem::ALL {
            assert_eq!(
                sys.default_unit(d).dimension(),
                d,
                "{}'s {} default must belong to it",
                d.as_str(),
                sys.as_str()
            );
        }
    }
}

/// Tokens and abbreviations must be UNIQUE across the whole vocabulary — a duplicate token would
/// make `parse` return the wrong unit (it takes the first match), which is a silent wrong number.
#[test]
fn unit_tokens_are_unique() {
    let mut tokens: Vec<&str> = Unit::ALL.iter().map(|u| u.as_str()).collect();
    tokens.sort_unstable();
    let before = tokens.len();
    tokens.dedup();
    assert_eq!(before, tokens.len(), "duplicate unit token");

    let mut dims: Vec<&str> = Dimension::ALL.iter().map(|d| d.as_str()).collect();
    dims.sort_unstable();
    let before = dims.len();
    dims.dedup();
    assert_eq!(before, dims.len(), "duplicate dimension token");
}
