//! Opening the on-disk engine — and saying something useful when it will not open.
//!
//! Two things that are about the OPEN itself rather than the `Store` API, which is why they are not
//! in `open.rs`: waiting out a directory lock the previous holder has not released yet, and
//! recognising a store written by the previous storage engine.

use surrealdb::engine::local::{Db, SurrealKv};
use surrealdb::Surreal;

use crate::open::StoreError;

/// How long to keep retrying an open that is blocked by the previous holder's directory lock.
/// Measured release is ~150 ms (`store/tests/store_lock_probe.rs`); five seconds is ~30x that, so it
/// absorbs a loaded box without turning a genuinely-held lock into a long hang.
const LOCK_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Open the on-disk engine, waiting out a directory lock the previous holder has not yet released.
///
/// surrealkv 0.21 takes an exclusive lock on the store directory and does NOT release it
/// synchronously when the handle drops — an immediate reopen of the same path fails with
/// "Database at <path>/store/LOCK is already locked by another process". That is a race, not a
/// permanent state: `store_lock_probe.rs` measures the release at ~150 ms, and shows that a LOCK
/// left behind by a killed process does NOT block a later open (a clean close removes the file, and
/// a forged stale one is opened straight through). So a node restart is never bricked — but a
/// restart quick enough to beat the old handle's release would fail for no good reason, and five
/// `lb-host` tests plus the boot guard hit exactly that.
///
/// Retrying is therefore right, and bounded: a lock genuinely held by a LIVE second process must
/// still surface as an error rather than hang, which is what the deadline gives us.
pub(crate) async fn open_engine_awaiting_lock(path: &str) -> Result<Surreal<Db>, StoreError> {
    let deadline = std::time::Instant::now() + LOCK_WAIT;
    let mut attempts = 0u32;
    loop {
        attempts += 1;
        match Surreal::new::<SurrealKv>(path).await {
            Ok(db) => {
                if attempts > 1 {
                    tracing::info!(
                        path = %path,
                        attempts,
                        "store opened after waiting out the previous holder's directory lock"
                    );
                }
                return Ok(db);
            }
            // Match on the message because surrealkv surfaces this through SurrealDB as an opaque
            // `Other` error with no typed variant to match — checked against surrealdb 3.2.4.
            Err(e)
                if e.to_string().contains("is already locked")
                    && std::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
            Err(e) => return Err(explain_old_format(path, e.into())),
        }
    }
}

/// Turn "the engine would not open this directory" into something an operator can act on.
///
/// surrealkv 0.9 (SurrealDB 2) wrote a bitcask log into `clog/`; 0.21 is an LSM tree and writes
/// `wal/`, `sstables/`, `vlog/` and `manifest/`. It cannot read the old layout, so an upgraded node
/// fails to open its existing store — correctly, since serving an empty workspace would be worse.
/// What it says on its own is `IO error: File exists`, which tells an operator nothing.
///
/// A `clog/` directory is an unambiguous marker: only the old engine ever created one.
fn explain_old_format(path: &str, e: StoreError) -> StoreError {
    if !std::path::Path::new(path).join("clog").is_dir() {
        return e;
    }
    StoreError::Backend(format!(
        "the store at {path} was written by surrealkv 0.9 (SurrealDB 2) — it has a `clog/` \
         directory, which only that engine created. surrealkv 0.21 is a different on-disc format \
         and cannot read it, so this node will NOT start against it. Nothing has been modified. \
         Either point the node at a fresh directory, or move `clog/` aside once you have exported \
         anything you still need from the old build. Underlying error: {e}"
    ))
}
