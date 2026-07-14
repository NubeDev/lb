//! Measure a directory: bytes on disk + how recently anything in it was touched.
//!
//! Both numbers come from ONE walk. `last_used` is the newest mtime found anywhere
//! in the tree, which is what makes age-based policy honest: a `target/` dir whose
//! newest artifact is 60 days old is genuinely cold, no matter what the mtime of the
//! top directory inode happens to say (it barely moves once the dir exists).
//!
//! ## Hardlinks are counted once
//!
//! Cargo `target/` dirs are full of hardlinks — the same inode appears as
//! `debug/foo` and `debug/deps/foo-<hash>`. Naively summing `len()` per directory
//! entry counted this box's `lb` target as 145 GB when `du` (and reality) say 84 GB:
//! a 1.7x over-report. Deleting the tree only frees an inode's blocks ONCE, so the
//! naive number promises disk we cannot deliver — the exact direction that must
//! never happen, since it is what the tray shows and what you decide on.
//!
//! So: any file with `nlink > 1` is charged only the first time its `(dev, ino)` is
//! seen in this walk. We track only multiply-linked inodes, not every file, keeping
//! the set tiny.
//!
//! Still apparent size, not allocated blocks: a sparse file is charged its nominal
//! length. Build artifacts are not sparse, so this does not bite in practice.

use std::collections::HashSet;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use walkdir::WalkDir;

/// What one walk learned about a directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Measured {
    /// Sum of file sizes in bytes, counting each hardlinked inode once.
    pub bytes: u64,
    /// Newest mtime anywhere in the tree, as seconds since the unix epoch.
    /// `0` means the tree is empty or no mtime was readable.
    pub last_used_secs: u64,
}

/// Walk `dir` and total it up.
///
/// Never follows symlinks — a symlink into another tree would double-count it, and
/// a symlink loop would hang. Unreadable entries are skipped rather than failing the
/// whole scan: a permission error deep in a cache must not blind us to the 90 GB
/// sitting next to it.
pub fn measure(dir: &Path) -> Measured {
    let mut bytes = 0u64;
    let mut last_used_secs = 0u64;
    // Only multiply-linked inodes need tracking; singly-linked files can't repeat.
    let mut seen_inodes: HashSet<(u64, u64)> = HashSet::new();

    for entry in WalkDir::new(dir).follow_links(false).into_iter().flatten() {
        let Ok(meta) = entry.metadata() else { continue };

        if meta.is_file() && charge_this_file(&meta, &mut seen_inodes) {
            bytes = bytes.saturating_add(meta.len());
        }
        if let Some(secs) = mtime_secs(&meta) {
            last_used_secs = last_used_secs.max(secs);
        }
    }

    Measured {
        bytes,
        last_used_secs,
    }
}

/// Should this file's bytes count? Yes, unless we already charged its inode.
fn charge_this_file(meta: &std::fs::Metadata, seen_inodes: &mut HashSet<(u64, u64)>) -> bool {
    if meta.nlink() <= 1 {
        return true;
    }
    seen_inodes.insert((meta.dev(), meta.ino()))
}

fn mtime_secs(meta: &std::fs::Metadata) -> Option<u64> {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
}

/// Seconds since the unix epoch, now.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sums_nested_files_and_takes_the_newest_mtime() {
        let root = tempfile::tempdir().unwrap();
        let nested = root.path().join("a/b");
        std::fs::create_dir_all(&nested).unwrap();

        let mut f = std::fs::File::create(nested.join("blob.bin")).unwrap();
        f.write_all(&[0u8; 2048]).unwrap();
        f.sync_all().unwrap();

        let m = measure(root.path());
        assert_eq!(m.bytes, 2048, "nested file must be counted");
        assert!(m.last_used_secs > 0, "an mtime must have been read");
        assert!(
            m.last_used_secs <= now_secs() + 1,
            "mtime must not be in the future"
        );
    }

    /// The 1.7x over-report bug: `lb`'s target measured 145 GB against `du`'s 84 GB
    /// because cargo hardlinks artifacts. Deleting frees the blocks ONCE, so we must
    /// charge them once — never promise disk we cannot deliver.
    #[test]
    fn a_hardlinked_inode_is_counted_once_not_per_link() {
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("libfoo.rlib");
        std::fs::write(&original, vec![0u8; 4096]).unwrap();

        // What cargo does: the same artifact linked under deps/ with a hashed name.
        let deps = root.path().join("deps");
        std::fs::create_dir_all(&deps).unwrap();
        std::fs::hard_link(&original, deps.join("libfoo-1a2b3c.rlib")).unwrap();
        std::fs::hard_link(&original, deps.join("libfoo-4d5e6f.rlib")).unwrap();

        assert_eq!(
            measure(root.path()).bytes,
            4096,
            "3 links to one 4 KB inode = 4 KB of disk, not 12 KB"
        );
    }

    /// Distinct files that merely share a size must still both count — guard against
    /// the dedup being too aggressive.
    #[test]
    fn two_separate_files_both_count() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("a.bin"), vec![0u8; 1024]).unwrap();
        std::fs::write(root.path().join("b.bin"), vec![0u8; 1024]).unwrap();

        assert_eq!(measure(root.path()).bytes, 2048);
    }

    #[test]
    fn a_symlink_does_not_double_count_its_target() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("real.bin"), vec![0u8; 1024]).unwrap();
        std::os::unix::fs::symlink(root.path().join("real.bin"), root.path().join("link.bin"))
            .unwrap();

        assert_eq!(measure(root.path()).bytes, 1024);
    }

    #[test]
    fn empty_dir_measures_zero_bytes() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(measure(root.path()).bytes, 0);
    }

    #[test]
    fn missing_dir_is_not_a_panic() {
        let m = measure(Path::new("/definitely/does/not/exist/anywhere"));
        assert_eq!(m.bytes, 0, "an unreadable root yields zero, never a panic");
    }
}
