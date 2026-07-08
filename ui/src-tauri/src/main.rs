//! The desktop entry point. With the `desktop` feature it boots the node, registers the IPC
//! commands as Tauri `#[command]`s, and opens the window. Without it (the default — so the
//! command layer builds + tests on a machine with no webkit toolchain), it prints how to run
//! the windowed build and exits. Role/window selection is config, not core code (§3.1).

// Windows: GUI subsystem in release builds so no console window opens behind the app.
#![cfg_attr(all(not(debug_assertions), windows), windows_subsystem = "windows")]

#[cfg(not(feature = "desktop"))]
fn main() {
    eprintln!(
        "lazybones-shell: built without the `desktop` feature (no window).\n\
         The IPC command layer is in the library (tested via `cargo test -p lazybones-shell`).\n\
         To run the desktop window on a machine with the webkit toolchain:\n  \
         cargo run -p lazybones-shell --features desktop"
    );
}

#[cfg(feature = "desktop")]
mod desktop;

#[cfg(feature = "desktop")]
fn main() {
    desktop::run();
}
