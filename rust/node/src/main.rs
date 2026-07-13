//! The `node` binary entry point. Selects roles by config (README §9) and drives the host, but owns
//! **no** boot logic itself: it fills a [`BootConfig`](lb_node::BootConfig) from env at the binary
//! boundary, calls [`boot_full`](lb_node::boot_full) — the one shared boot ritual in the `lb_node` lib
//! — and serves. Behaviour is identical to before this seam existed (same env vars, same seeds, same
//! gateway); the ritual just lives in the library now so the Tauri shell, the test gateway, and a
//! third-party embedder all call the exact same code (embed scope). Kept to one verb (FILE-LAYOUT).

use lb_node::{boot_full, BootConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Fill the boot config from env at the binary boundary (the ONLY place `LB_*` boot vars are read),
    // perform the full ritual once via the lib seam, then serve the gateway (a no-op when the gateway
    // is off — a solo/headless node just runs its reactors and in-process verbs).
    let running = boot_full(BootConfig::from_env()).await?;
    running.serve().await
}
