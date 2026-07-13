//! Generate the guest `wit_bindgen::generate!` against the authoritative WIT owned by the standalone
//! `lb-sdk` crate. `generate!` resolves `path:` against this crate's manifest dir, so with the SDK now
//! a dependency (not a sibling `../../sdk`), we read the WIT's absolute location from `lb-sdk`'s
//! `links` build metadata (`DEP_LB_SDK_WIT`) and emit the macro call with that path baked in as a
//! literal, `include!`-ed by `src/lib.rs`. One authoritative WIT — the guest and host generate from
//! the exact same source, so the ABI cannot drift.

use std::path::Path;

fn main() {
    let wit = std::env::var("DEP_LB_SDK_WIT")
        .expect("DEP_LB_SDK_WIT — lb-sdk must be a dependency exporting its WIT via `links`");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");
    let gen = format!(
        r#"wit_bindgen::generate!({{
    path: {wit:?},
    world: "extension",
}});"#
    );
    std::fs::write(Path::new(&out).join("wit_gen.rs"), gen).expect("write wit_gen.rs");
    println!("cargo:rerun-if-env-changed=DEP_LB_SDK_WIT");
}
