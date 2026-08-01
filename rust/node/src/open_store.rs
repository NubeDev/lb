//! `open_store` — turn the boot config's store selection into a live [`lb_store::Store`].
//!
//! The ONE place `BootConfig::store_path` becomes a store: `Some(non-empty)` ⇒ a durable on-disk
//! open, `None`/empty ⇒ an ephemeral `mem://` one. Config, never a role branch (rule 1) — an edge
//! node and a cloud node run this identical line and differ only in the string.
//!
//! It is also where the **boot memory guard** (issue #128) is configured, and where its refusal
//! becomes a fatal boot error.

use crate::config::BootConfig;

/// Open the store the boot config selects: `store_path: Some(non-empty)` ⇒ a durable on-disk store;
/// `None`/empty ⇒ an ephemeral `mem://` store. This is the ONE place the store path (today's
/// `LB_STORE_PATH`, filled into `cfg` at the binary boundary) turns into a `Store` — no library code
/// below reads the env. Mirrors `Node::open_store`'s config-not-role selection, but sourced from the
/// struct so an embedder controls it directly.
pub(crate) async fn open_store(cfg: &BootConfig) -> anyhow::Result<lb_store::Store> {
    let store = match cfg.store_path.as_deref() {
        Some(path) if !path.is_empty() => {
            // The boot memory guard may REFUSE this open (`StoreError::WontFit`) when the commit
            // log cannot fit in this machine's RAM. That error propagates out of boot and exits the
            // binary nonzero with the diagnostic — it must NEVER fall back to `mem://`: a silently
            // empty node serving a workspace that "lost" its data is strictly worse than a down
            // node with a legible reason (boot-memory-guard scope, decision 3).
            let opts = lb_store::OpenOptions::default()
                .allow_unguarded(cfg.store_open_unguarded)
                .with_available_ram(cfg.store_available_ram_bytes);
            lb_store::Store::open_with(path, &opts).await?
        }
        _ => lb_store::Store::memory().await?,
    };
    Ok(store)
}
