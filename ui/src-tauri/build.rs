// Tauri's build step runs ONLY for the windowed (`desktop`) build — the headless command-layer
// build/test needs no codegen and no webkit toolchain. Gating it on the feature keeps
// `cargo test -p lazybones-shell` toolchain-free. `tauri-build` is pure-Rust codegen (no
// system deps), so it is always present; we just skip calling it unless the feature is on.
fn main() {
    if std::env::var("CARGO_FEATURE_DESKTOP").is_ok() {
        tauri_build::build();
    }
}
