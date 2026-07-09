//! Desktop `full` boot: resolve a DURABLE per-user store path so a restart keeps the user's work
//! (desktop-persistent-store scope). Without this, `Node::boot`→`open_store()` sees an unset
//! `LB_STORE_PATH` and opens an EPHEMERAL in-memory store — every relaunch loses channels,
//! dashboards, datasources, everything. This resolves the OS-standard per-user data dir and lets the
//! windowed boot fill `LB_STORE_PATH` with it, so `open_store` opens the persistent SurrealKV engine
//! it already supports — no core change, just config (§3.1).
//!
//! **Set at the windowed BINARY boundary, never in `Node::boot`/`NodeHandle::boot`.** Those are
//! called directly by the command + integration tests, which need isolated in-memory stores; baking
//! a default path there would make every test share one on-disk store (cross-contamination). Only the
//! shipped app takes the `desktop::run` path — the correct place for a shipped-app default.

/// The app's data-dir subfolder (`<os-data-dir>/lazybones/store`).
const APP_DIR: &str = "lazybones";
const STORE_DIR: &str = "store";

/// Resolve the default persistent store path under the user's OS data directory, creating the parent
/// dir. Returns `None` only if the data dir can't be resolved AND the fallback can't be created — the
/// caller then leaves `LB_STORE_PATH` unset (ephemeral), which is logged loudly so the data-loss is
/// never silent.
///
/// - Windows → `%APPDATA%\lazybones\store`
/// - Linux   → `~/.local/share/lazybones/store`
/// - macOS   → `~/Library/Application Support/lazybones/store`
pub fn resolve_store_path() -> Option<String> {
    // The per-user data dir is the primary choice; the exe-adjacent `./store` is a fallback for an
    // unusual environment where `dirs::data_dir()` is `None` (e.g. a headless container). Never fall
    // straight to in-memory here — that would quietly reproduce the "lost my work" bug.
    let base = dirs::data_dir()
        .map(|d| d.join(APP_DIR).join(STORE_DIR))
        .or_else(|| {
            std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|p| p.join(STORE_DIR)))
        })?;

    // `Store::open` (SurrealKV) creates the store dir itself; we only need its PARENT to exist.
    if let Some(parent) = base.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "full: could not create store parent dir {}: {e}",
                parent.display()
            );
            return None;
        }
    }
    Some(base.to_string_lossy().into_owned())
}

/// Fill `LB_STORE_PATH` with the resolved per-user default IF it is unset/empty, so `Node::boot`
/// opens a durable store. An explicit `LB_STORE_PATH` (a portable/USB layout, a custom dir, a test)
/// wins untouched; an explicit EMPTY value is honored as "ephemeral on purpose" and left alone.
/// Returns the effective mode string for logging. Call ONCE, before `NodeHandle::boot`.
pub fn ensure_store_path() -> String {
    match std::env::var("LB_STORE_PATH") {
        // Set (non-empty) → persistent at the caller's path; don't override.
        Ok(p) if !p.is_empty() => format!("persistent store at {p} (LB_STORE_PATH)"),
        // Set-but-empty → an explicit ephemeral opt-out; honor it.
        Ok(_) => "in-memory store (LB_STORE_PATH is empty — ephemeral by request)".to_string(),
        // Unset → the shipped-app default: resolve the per-user data dir and set it.
        Err(_) => match resolve_store_path() {
            Some(path) => {
                std::env::set_var("LB_STORE_PATH", &path);
                format!("persistent store at {path}")
            }
            None => "in-memory store (could NOT resolve a data dir — work will NOT persist!)"
                .to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_yields_a_lazybones_store_path() {
        // In any environment with a data dir OR an exe path, the resolved path ends in the store dir.
        if let Some(p) = resolve_store_path() {
            assert!(
                p.ends_with(STORE_DIR),
                "resolved path ends with the store dir: {p}"
            );
        }
    }
}
