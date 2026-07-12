//! Locate + stat an extension's concrete build outputs, so an inspect can prove a build wrote a
//! fresh artifact (not just that a `target/…/release` dir exists — a stale artifact reads as "built"
//! forever otherwise). The UI snapshots these before a build and diffs size/mtime against a fresh
//! inspect after; see `devkit-container-build-scope.md`.
//!
//! We enumerate the release dir rather than guessing `{crate}.wasm` from the manifest: the crate
//! name can differ from the extension id, and enumeration also survives a rename without a code
//! change. mtime uses the same RFC3339-seconds format as `host.fs.stat`, so the two are comparable.

use std::fs::{self, Metadata};
use std::path::Path;
use std::time::UNIX_EPOCH;

use crate::model::BuildArtifact;
use crate::Tier;

/// Every build output currently on disk for `path`, sorted by kind then path for a stable order.
pub fn collect_artifacts(path: &Path, tier: Tier) -> Vec<BuildArtifact> {
    let mut out = Vec::new();

    // The compiled binary/component. wasm builds land under a wasm32-wasip2 triple; native under the
    // host triple's `release/`.
    let (release_dir, kind, ext) = match tier {
        Tier::Wasm => (path.join("target/wasm32-wasip2/release"), "wasm", "wasm"),
        Tier::Native => (path.join("target/release"), "native-bin", ""),
    };
    if let Ok(entries) = fs::read_dir(&release_dir) {
        for entry in entries.flatten() {
            let p = entry.path();
            let matches = match tier {
                Tier::Wasm => p.extension().is_some_and(|e| e == ext),
                // A native release dir is full of build scratch (deps/, *.d, incrementals); the
                // artifact is the executable at the top level with no extension.
                Tier::Native => p.is_file() && p.extension().is_none(),
            };
            if matches {
                if let Ok(meta) = entry.metadata() {
                    out.push(artifact(kind, &p, &meta));
                }
            }
        }
    }

    // The federated UI remote the gateway serves — the other half of "did the build work" for any
    // extension that ships a UI.
    let remote = path.join("ui/dist/remoteEntry.js");
    if let Ok(meta) = fs::metadata(&remote) {
        out.push(artifact("remote-entry", &remote, &meta));
    }

    out.sort_by(|a, b| (a.kind.as_str(), &a.path).cmp(&(b.kind.as_str(), &b.path)));
    out
}

fn artifact(kind: &str, path: &Path, meta: &Metadata) -> BuildArtifact {
    BuildArtifact {
        kind: kind.to_string(),
        path: path.to_path_buf(),
        size: meta.len(),
        mtime: mtime(meta),
    }
}

/// RFC3339 UTC, seconds precision — byte-for-byte the format `host.fs.stat` emits, so the UI can
/// compare a pre-build snapshot's mtime against a post-build one to confirm the artifact advanced.
fn mtime(meta: &Metadata) -> Option<String> {
    let modified = meta.modified().ok()?;
    let duration = modified.duration_since(UNIX_EPOCH).ok()?;
    let dt = chrono::DateTime::<chrono::Utc>::from(UNIX_EPOCH + duration);
    Some(dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}
