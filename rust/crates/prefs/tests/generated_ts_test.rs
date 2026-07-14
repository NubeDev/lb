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

use lb_prefs::axis::{Dimension, Unit};

#[test]
fn dimension_and_unit_counts_match_declared_all() {
    // Guard the ALL arrays against an enum variant added without updating ALL (the generator + every
    // exhaustive test reads ALL).
    assert_eq!(Dimension::ALL.len(), 8);
    assert_eq!(Unit::ALL.len(), 29);
    for u in Unit::ALL {
        assert_eq!(
            Unit::parse(u.as_str()),
            Some(u),
            "every unit token round-trips through parse"
        );
    }
}
