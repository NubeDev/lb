use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

/// Default to the repo's `rust/extensions` directory when called from either the repo root or `rust/`.
pub fn default_devkit_root() -> PathBuf {
    std::env::var_os("LB_DEVKIT_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            if cwd.join("extensions").is_dir() && cwd.join("crates").is_dir() {
                cwd.join("extensions")
            } else {
                cwd.join("rust/extensions")
            }
        })
}

pub fn resolve_under_root(root: impl AsRef<Path>, child: impl AsRef<Path>) -> Result<PathBuf> {
    let root = root.as_ref();
    std::fs::create_dir_all(root)
        .with_context(|| format!("create devkit root {}", root.display()))?;
    let root = root
        .canonicalize()
        .with_context(|| format!("canonicalize devkit root {}", root.display()))?;
    let child = child.as_ref();
    let joined = if child.is_absolute() {
        child.to_path_buf()
    } else {
        root.join(child)
    };
    reject_traversal(child)?;
    let parent = joined
        .parent()
        .ok_or_else(|| anyhow!("path has no parent: {}", joined.display()))?;
    std::fs::create_dir_all(parent)
        .with_context(|| format!("create parent {}", parent.display()))?;
    let parent = parent
        .canonicalize()
        .with_context(|| format!("canonicalize parent {}", parent.display()))?;
    if !parent.starts_with(&root) {
        bail!("path escapes LB_DEVKIT_ROOT");
    }
    Ok(parent.join(
        joined
            .file_name()
            .ok_or_else(|| anyhow!("path has no file name"))?,
    ))
}

fn reject_traversal(path: &Path) -> Result<()> {
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        bail!("path traversal is not allowed");
    }
    Ok(())
}
