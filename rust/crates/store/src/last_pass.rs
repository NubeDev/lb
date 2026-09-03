//! The persisted last-compaction record — a sub-KB JSON sidecar at `<store dir>/../last-compaction.json`
//! (boot-memory-guard scope slice 3, decision 4).
//!
//! `Store`'s `last_compaction` slot is in-memory, which is exactly backwards once boot may *skip*:
//! the skip decision needs the previous pass's outcome, and after a skip there is no fresh record to
//! serve. So a real pass writes its record beside the engine directory, and boot reads it back.
//!
//! **Beside, never inside.** `log_stats` sums `clog/*.clog` + `manifest` under the store dir, and the
//! `#122` budget arithmetic is built on that number; a foreign file inside the engine's directory
//! would also be a file the engine never expects. A sibling is neither.
//!
//! **Best-effort in both directions**, by contract: unreadable/corrupt/missing ⇒ `None` ⇒ the boot
//! precondition passes and the node compacts exactly as it does today; unwritable ⇒ warn and carry
//! on. Written atomically (tmp + rename in the same directory) so a crash mid-write can never leave
//! a half-file that a later boot would have to interpret.

use crate::compaction_record::CompactionRecord;

/// The sidecar's filename, resolved against the store directory's **parent**.
const LAST_COMPACTION_FILE: &str = "last-compaction.json";

/// Where the record for the store at `store_dir` lives: its sibling. A `store_dir` with no parent
/// component (a bare relative name) resolves against the current directory.
pub(crate) fn record_path(store_dir: &str) -> std::path::PathBuf {
    let dir = std::path::Path::new(store_dir);
    match dir.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.join(LAST_COMPACTION_FILE),
        _ => std::path::PathBuf::from(LAST_COMPACTION_FILE),
    }
}

/// Read the last **real** pass's record for the store at `store_dir`. Any failure — missing file,
/// unreadable, truncated, or JSON that no longer parses into the current shape — is `None`, which
/// the caller must treat as "no information", never as "nothing to reclaim".
pub(crate) fn load_last_compaction(store_dir: &str) -> Option<CompactionRecord> {
    let path = record_path(store_dir);
    let raw = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&raw) {
        Ok(rec) => Some(rec),
        Err(e) => {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "store: last-compaction record is unreadable — treating it as absent (the boot pass \
                 will run as it did before this file existed)"
            );
            None
        }
    }
}

/// The persisted record of the last pass that actually ran on `store`, if there is one. `None` for
/// a memory store (no log, no sidecar) and for any read failure.
///
/// This is how a **restart** recovers judgement it would otherwise lose: the `#122` budget driver
/// re-seeds its unproductive-suspension from it instead of starting every boot believing that
/// compaction pays here, and boot's own benefit precondition reads the same file.
pub fn last_persisted_compaction(store: &crate::open::Store) -> Option<CompactionRecord> {
    load_last_compaction(store.dir()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_lives_beside_the_store_dir() {
        assert_eq!(
            record_path("/var/lib/lb/store"),
            std::path::PathBuf::from("/var/lib/lb/last-compaction.json")
        );
        assert_eq!(
            record_path("store"),
            std::path::PathBuf::from("last-compaction.json")
        );
    }
}
