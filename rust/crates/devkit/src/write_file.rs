//! `write_file` — write/replace a single source file inside a scaffolded extension dir.
//!
//! The agent-driven authoring loop (scaffold → customize → build) needs a way for an agent that
//! reaches the platform ONLY through the MCP bridge to edit the source files `devkit.scaffold`
//! produced. This is that seam: it resolves `path` under the devkit root (the SAME
//! `resolve_under_root` gate `scaffold`/`build`/`inspect` use, so the same traversal / symlink-
//! escape guards apply), then writes `content` (UTF-8 text). It does not execute anything; it is
//! the file-write counterpart to `devkit.scaffold`.
//!
//! Scope: text source files only (`.rs`, `.tsx`, `.ts`, `.toml`, `.css`, `.json`, `.sh`, …). There
//! is no binary mode — extension source is text, and a binary surface would invite asset-stuffing
//! past the devkit root. The build step (`devkit.build`) is what turns the written source into a
//! signed artifact; `write_file` only feeds the loop.

use std::path::Path;

use anyhow::{Context, Result};

use crate::root::{default_devkit_root, resolve_under_root};
use crate::WriteFileReport;

/// Write `content` to the file at `path`, resolved under the devkit root (or the explicit
/// `root` override). Creates parent dirs as needed (via `resolve_under_root`); overwrites if the
/// file exists. Returns the canonical absolute path written + the byte count.
pub fn write_file(root: Option<&Path>, path: &Path, content: &str) -> Result<WriteFileReport> {
    let root = root
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_devkit_root);
    let resolved = resolve_under_root(&root, path)?;
    std::fs::write(&resolved, content).with_context(|| format!("write {}", resolved.display()))?;
    make_executable_if_script(&resolved)?;
    Ok(WriteFileReport {
        path: resolved,
        bytes: content.len() as u64,
    })
}

#[cfg_attr(not(unix), allow(unused_variables))]
fn make_executable_if_script(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        if path.file_name().and_then(|n| n.to_str()) == Some("build.sh") {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(path, perms)?;
        }
    }
    Ok(())
}
