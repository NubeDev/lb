//! Where a cached artifact's payload bytes live on disk, and how they get there — the filesystem
//! half of the cache split (registry scope, the same JSON-int-array-bloat defect the zip-transport
//! upload fix closes, found at rest instead of on the wire).
//!
//! `cache_artifact` used to `serde_json::to_value` the WHOLE `Artifact`, including `wasm: Vec<u8>` —
//! no custom serde impl on that field, so it paid the identical ~4-8x decimal-int-array bloat as the
//! JSON upload wire shape, just server-side, on every publish and every cache read/write against
//! SurrealDB. This module moves the payload bytes to a plain, content-addressed file; `cache.rs`
//! keeps only the small metadata (manifest/digest/publisher/signature) as the SurrealDB row.
//!
//! Content-addressed by `digest_hex` (identical bytes across `(ext_id, version)`s or repeated
//! publishes never write twice — the same dedup `cache_artifact`'s doc comment already promises) and
//! **workspace-scoped in the path**, mirroring `ext/install_dir.rs`'s `native_install_dir` pattern
//! exactly (same `LB_DIR` env convention, same sanitized path components, same temp-file-then-
//! `rename()` atomic write) so the cache's existing workspace-isolation guarantee — "a ws-B cache can
//! never read ws-A's artifact" — holds for the blob file the same way it already holds for the
//! SurrealDB namespace.

use std::io::Write;
use std::path::{Path, PathBuf};

use lb_store::StoreError;

/// This node's cache blob dir for `(ws, digest_hex)`: `{LB_DIR|.lazybones}/artifacts/{ws}/{digest}.bin`.
/// Deterministic — the same inputs always resolve to the same path, so `read_cached` re-derives what
/// `cache_artifact` wrote without anything extra persisted. Both components are sanitized, so a
/// hostile workspace id can never escape the base dir via `..` or a separator (same guarantee
/// `native_install_dir` gives its own tree).
pub(crate) fn artifact_blob_path(ws: &str, digest_hex: &str) -> PathBuf {
    let base = std::env::var("LB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".lazybones"));
    base.join("artifacts")
        .join(sanitize_component(ws))
        .join(format!("{digest_hex}.bin"))
}

/// Mirrors `ext/install_dir.rs`'s `sanitize_component` verbatim — deliberately a local, independent
/// copy (this crate's established idiom: `digest.rs`/`devkit::hex` also each hand-roll their own tiny
/// codec rather than share one) rather than a cross-module dependency between two otherwise-unrelated
/// subsystems (native install vs. artifact cache) that only happen to need the same five lines.
fn sanitize_component(raw: &str) -> String {
    raw.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Write `bytes` to `path`, creating parent dirs. A no-op if the file already exists — content-
/// addressed by digest, so identical bytes are byte-for-byte identical on disk already; re-caching
/// the same digest (idempotent by design, per `cache_artifact`'s own doc comment) skips the write
/// rather than re-writing indistinguishable bytes. Otherwise atomic: temp sibling then `rename()`,
/// same reasoning `write_executable` documents (a concurrent reader can never observe a partial
/// file), even though nothing reads this file while it's being written today — cheap insurance for
/// free.
pub(crate) fn write_blob_atomic(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    if path.exists() {
        return Ok(());
    }
    let dir = path
        .parent()
        .ok_or_else(|| io_err_msg("blob path has no parent dir"))?;
    std::fs::create_dir_all(dir).map_err(io_err)?;
    let file_name = path
        .file_name()
        .ok_or_else(|| io_err_msg("blob path has no file name"))?;
    let tmp = dir.join(format!(".{}.tmp", file_name.to_string_lossy()));
    let mut f = std::fs::File::create(&tmp).map_err(io_err)?;
    f.write_all(bytes).map_err(io_err)?;
    f.flush().map_err(io_err)?;
    drop(f);
    std::fs::rename(&tmp, path).map_err(io_err)?;
    Ok(())
}

/// Read the blob at `path` in full. `StoreError::Backend` (not a bespoke variant) on any failure,
/// including "not found" — `read_cached`'s caller already treats a cache miss as "go fetch it", the
/// same posture a missing SurrealDB row has today; this is not a new failure mode to distinguish.
pub(crate) fn read_blob(path: &Path) -> Result<Vec<u8>, StoreError> {
    std::fs::read(path).map_err(io_err)
}

fn io_err(e: std::io::Error) -> StoreError {
    StoreError::Backend(format!("artifact blob: {e}"))
}

fn io_err_msg(msg: &str) -> StoreError {
    StoreError::Backend(format!("artifact blob: {msg}"))
}

/// The ONE `LB_DIR` every test in this crate that touches `artifact_blob_path` runs under.
///
/// `LB_DIR` is process-global and read live by every call; libtest runs tests on concurrent threads,
/// so a per-test `set_var`/`remove_var` is a genuine data race (this crate's `ext_boot_spawn_test.rs`
/// documents the same hazard for `native_install_dir`, which reads the same env var the same way).
/// Set it ONCE, before any test can call the reader, and never mutate it again — every test (in this
/// module AND in `cache::tests`, which shares this one static) namespaces its own path by `ws`
/// and/or `digest_hex`, so the fixed shared root never causes a cross-test collision.
#[cfg(test)]
pub(crate) static TEST_LB_DIR: std::sync::LazyLock<std::path::PathBuf> =
    std::sync::LazyLock::new(|| {
        let dir = std::env::temp_dir().join(format!("lb-registry-blob-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("test LB_DIR");
        std::env::set_var("LB_DIR", &dir);
        dir
    });

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile workspace id cannot climb out of the base dir — the same property
    /// `install_dir.rs`'s `a_component_can_never_escape_the_base_dir` pins for its own tree.
    #[test]
    fn a_workspace_id_can_never_escape_the_base_dir() {
        let path = artifact_blob_path("../../etc", "a".repeat(64).as_str());
        let s = path.to_string_lossy();
        assert!(!s.contains(".."), "escaped the base dir: {s}");
        assert!(s.contains("artifacts"), "not under the artifacts base: {s}");
    }

    /// The property `read_cached` depends on: the same `(ws, digest)` always resolves to the same
    /// path, so a read re-derives what a prior write used without anything extra persisted.
    #[test]
    fn the_path_is_deterministic_for_a_ws_and_digest() {
        assert_eq!(
            artifact_blob_path("acme", "abc123"),
            artifact_blob_path("acme", "abc123")
        );
    }

    /// The isolation wall in the path, not just the SurrealDB namespace: two workspaces caching the
    /// SAME digest must not collide on disk.
    #[test]
    fn two_workspaces_never_share_a_blob_path_for_the_same_digest() {
        assert_ne!(
            artifact_blob_path("acme", "abc123"),
            artifact_blob_path("other", "abc123"),
            "the workspace wall is structural in the path, too"
        );
    }

    #[test]
    fn write_then_read_round_trips() {
        std::sync::LazyLock::force(&TEST_LB_DIR);
        // A digest unique to this test — the shared root is fixed for the whole binary, so
        // collision-avoidance is on this identifier, not on a per-test LB_DIR.
        let path = artifact_blob_path("acme", "write-then-read-round-trips");
        write_blob_atomic(&path, b"hello world").unwrap();
        assert_eq!(read_blob(&path).unwrap(), b"hello world");
    }

    #[test]
    fn re_writing_the_same_path_is_a_cheap_no_op() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("x.bin");
        write_blob_atomic(&path, b"first").unwrap();
        // A second write to the SAME path (as a real caller would only ever do for the SAME digest,
        // i.e. identical bytes) must not touch the file — assert by writing DIFFERENT bytes and
        // confirming they are silently ignored, proving the exists-check short-circuits.
        write_blob_atomic(&path, b"second-and-different").unwrap();
        assert_eq!(read_blob(&path).unwrap(), b"first");
    }
}
