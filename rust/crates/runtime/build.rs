//! Locate the authoritative WIT (owned by the `lb-sdk` crate, now a standalone dependency) and emit
//! the host-side `bindgen!` invocations that read it.
//!
//! `wasmtime::component::bindgen!` resolves its `path:` against the *consuming* crate's manifest dir,
//! so once `lb-sdk` is a git/registry dep instead of a sibling `../../sdk` dir, this crate can no
//! longer name the WIT by a relative path. `lb-sdk`'s `build.rs` (a `links` crate) exports the WIT's
//! absolute location as `DEP_LB_SDK_WIT` / `DEP_LB_SDK_WIT_COMPAT`; we read those here and generate
//! two tiny modules that call `bindgen!` with the ABSOLUTE path baked in as a literal (an absolute
//! `path:` is used verbatim — `CARGO_MANIFEST_DIR.join(abs) == abs`). `bindings.rs` / `compat_v0_1.rs`
//! then `include!` the generated file. One authoritative WIT, reached without a copy in this repo.

use std::path::Path;

fn main() {
    let wit = std::env::var("DEP_LB_SDK_WIT")
        .expect("DEP_LB_SDK_WIT — lb-sdk must be a direct dependency exporting its WIT via `links`");
    let wit_compat = std::env::var("DEP_LB_SDK_WIT_COMPAT")
        .expect("DEP_LB_SDK_WIT_COMPAT — lb-sdk must export its 0.1 compat WIT via `links`");
    let out = std::env::var("OUT_DIR").expect("OUT_DIR");

    // The @0.2.0 host bindings: both the export (`tool.call`) and the `host.call-tool` import are
    // async (the callback forwards to the host's async dispatch chokepoint). This is the verbatim
    // invocation that lived in `bindings.rs`, with the path now sourced from the SDK crate.
    let bindings = format!(
        r#"wasmtime::component::bindgen!({{
    path: {wit:?},
    world: "extension",
    exports: {{ default: async }},
    imports: {{ default: async }},
}});"#
    );
    std::fs::write(Path::new(&out).join("bindings_gen.rs"), bindings).expect("write bindings_gen.rs");

    // The frozen @0.1.0 world snapshot, linked alongside so existing 0.1 guests still load.
    let compat = format!(
        r#"wasmtime::component::bindgen!({{
    path: {wit_compat:?},
    world: "extension",
    exports: {{ default: async }},
}});"#
    );
    std::fs::write(Path::new(&out).join("compat_gen.rs"), compat).expect("write compat_gen.rs");

    println!("cargo:rerun-if-env-changed=DEP_LB_SDK_WIT");
    println!("cargo:rerun-if-env-changed=DEP_LB_SDK_WIT_COMPAT");
}
