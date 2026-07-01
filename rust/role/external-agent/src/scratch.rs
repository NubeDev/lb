//! The **per-run scratch-dir seal** (run-lifecycle #5 "Code gap today"): every external-agent run
//! gets its **own** scratch dir + cwd, never a shared one. This is the filesystem half of the
//! zero-cross-workspace-bleed invariant, and it is local + testable now (the walled MCP endpoint +
//! scoped token — the data + secret seals — land with #3/#4).
//!
//! The layout is `{base}/lb-external-agent/{ws}/{job_id}/`, so:
//! - two runs in the **same** workspace still get **separate** dirs (they don't stomp each other);
//! - a `ws=A` run's dir is under `…/A/…` and a `ws=B` run's under `…/B/…` — distinct roots, so
//!   nothing a run writes can appear in another workspace's tree.
//!
//! **This is not yet the OS sandbox** (#3 owns the kernel-level egress/fs confinement). It is the
//! *cwd + scratch* seal the driver hands the subprocess as its working directory; #3 will additionally
//! confine the process to this dir at the kernel level. The seam is here so #3 wraps it, not replaces
//! it.

use std::io;
use std::path::PathBuf;

/// A per-run scratch directory. Dropped without deletion here — retention/reaping is #5's job (the
/// job's terminal handler tears down the dir + sandbox). Holding the path (not deleting on drop)
/// keeps this seam pure filesystem-layout; supervision owns lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    /// Construct (creating on disk) the scratch dir for run `job_id` in workspace `ws`, under `base`
    /// (the node's configured scratch root — defaults to the OS temp dir). Path components are
    /// sanitized so a hostile `ws`/`job_id` cannot escape the base with `..` or separators.
    pub fn create(base: &std::path::Path, ws: &str, job_id: &str) -> io::Result<Self> {
        let path = base
            .join("lb-external-agent")
            .join(sanitize(ws))
            .join(sanitize(job_id));
        std::fs::create_dir_all(&path)?;
        Ok(Self { path })
    }

    /// The scratch dir path — the subprocess's cwd (and, with #3, its only writable fs root).
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
}

/// Reduce an untrusted key to a single safe path component: keep alnum/`-`/`_`, map everything else
/// (including `/`, `\`, `.`, `:`) to `_`. So `../../etc` becomes `______etc` — it can never climb out
/// of `base`. Empty input becomes `_` so a dir always exists.
fn sanitize(s: &str) -> String {
    let mapped: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if mapped.is_empty() {
        "_".to_string()
    } else {
        mapped
    }
}

/// The default scratch root when the node config supplies none: the OS temp dir. A node may point
/// this at a dedicated volume (config, `either` placement — never a code branch).
pub fn default_scratch_base() -> PathBuf {
    std::env::temp_dir()
}
