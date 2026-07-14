//! The built-in MF1 catalogs must parse and stay key-for-key aligned, so the es→en fallback is
//! total — a key present in one builtin but not the other renders as a bare key for some locale.
//!
//! This file once also drift-tested the generated client twin
//! (`ui/src/lib/prefs/catalog.generated.ts`) for byte-identity against the generator. That guard
//! died with the in-tree client (`678503f` "deleted the ui") — lb is a library now and the
//! consuming client lives out of tree, so there is no checked-in file to be identical *to*.
//! Re-pointing it at a regenerated file would only assert the generator matches itself. The
//! generator (`cargo run -p lb-prefs --bin gen-prefs-catalog`) remains the way to emit the twin;
//! a consumer that vendors it owns its own drift test.

use lb_prefs::catalog::{parse_builtin, EN_MF, ES_MF};

#[test]
fn builtins_parse_and_share_keys() {
    // Both builtins parse, carry the same version, and are key-for-key aligned (so the es→en
    // fallback is total — no en key is missing an es counterpart or vice versa).
    let en = parse_builtin(EN_MF);
    let es = parse_builtin(ES_MF);
    assert_eq!(en.version, "1");
    assert_eq!(es.version, "1");
    assert!(!en.messages.is_empty());
    assert_eq!(
        en.messages.keys().collect::<Vec<_>>(),
        es.messages.keys().collect::<Vec<_>>(),
        "en and es builtins must share the same key set"
    );
}
