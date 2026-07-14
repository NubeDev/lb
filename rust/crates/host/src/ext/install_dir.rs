//! Where a native extension's binary lands on this node, and how it gets there — the one place the
//! `(ws, ext)` → install-dir mapping and the executable write live (lifecycle-management scope).
//!
//! Shared by the two paths that bring a native extension online, so they cannot drift: `ext.publish`
//! (the interactive upload) and `boot_spawn` (the boot respawn). That sharing is load-bearing rather
//! than tidiness — boot must land the binary in **exactly** the directory publish used, and it
//! re-derives that path from `(ws, ext)` instead of persisting it. Two copies of this rule would mean
//! a published extension that silently fails to respawn (issue #64's neighbourhood).

use std::io::Write;
use std::path::PathBuf;

use super::error::ExtError;

/// This node's install dir for `(ws, ext)`: `{LB_DIR|.lazybones}/native/{ws}/{ext}`. Deterministic —
/// the same inputs always resolve to the same directory, which is what lets boot find a previously
/// published binary's home without storing it. Both components are sanitized, so an exotic workspace
/// or extension id can never escape the base dir via `..` or a separator.
pub(crate) fn native_install_dir(ws: &str, ext: &str) -> PathBuf {
    let base = std::env::var("LB_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".lazybones"));
    base.join("native")
        .join(sanitize_component(ws))
        .join(sanitize_component(ext))
}

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

/// Write `bytes` as an executable file `dir/name` (creating `dir`).
///
/// Write to a temp sibling then RENAME into place: a re-publish over a RUNNING sidecar cannot open
/// the mapped binary for write (`ETXTBSY` — "Text file busy"), but a rename replaces the directory
/// entry without touching the executing inode, so the swap is atomic and the old child keeps running
/// its (unlinked) image until `install_native` stops it.
pub(crate) fn write_executable(dir: &PathBuf, name: &str, bytes: &[u8]) -> Result<(), ExtError> {
    std::fs::create_dir_all(dir).map_err(io_err)?;
    let path = dir.join(name);
    let tmp = dir.join(format!(".{name}.tmp"));
    let mut f = std::fs::File::create(&tmp).map_err(io_err)?;
    f.write_all(bytes).map_err(io_err)?;
    f.flush().map_err(io_err)?;
    drop(f);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(io_err)?;
    }
    std::fs::rename(&tmp, &path).map_err(io_err)?;
    Ok(())
}

fn io_err(e: std::io::Error) -> ExtError {
    ExtError::Native(format!("writing native binary: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A hostile workspace or extension id cannot climb out of the base dir — the sanitizer is the
    /// only thing between an id (which reaches us from a manifest) and an arbitrary write path.
    #[test]
    fn a_component_can_never_escape_the_base_dir() {
        let dir = native_install_dir("../../etc", "../../../bin/sh");
        let s = dir.to_string_lossy();
        assert!(!s.contains(".."), "escaped the base dir: {s}");
        assert!(s.contains("native"), "not under the native base: {s}");
    }

    /// The property boot depends on: the same `(ws, ext)` always resolves to the same directory, so
    /// boot re-derives publish's path without anything being persisted.
    #[test]
    fn the_dir_is_deterministic_for_a_ws_and_ext() {
        assert_eq!(
            native_install_dir("acme", "echo-sidecar"),
            native_install_dir("acme", "echo-sidecar")
        );
        assert_ne!(
            native_install_dir("acme", "echo-sidecar"),
            native_install_dir("other", "echo-sidecar"),
            "the workspace wall is structural in the path, too"
        );
    }
}
