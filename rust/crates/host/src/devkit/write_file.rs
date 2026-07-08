//! `devkit.write_file` — write/replace one source file inside a scaffolded extension dir.
//!
//! The agent authoring loop (scaffold → customize → build) needs a file-write seam an
//! MCP-only agent can reach. This is the host gate around `lb_devkit::write_file`: it runs the
//! same `mcp:devkit.write_file:call` cap check every other devkit verb does, then delegates to
//! `lb_devkit::write_file`, which resolves the path under the devkit root with the same
//! `resolve_under_root` traversal/symlink guards `scaffold`/`build`/`inspect` use.

use std::path::Path;

use lb_auth::Principal;

use super::{authorize_devkit, DevkitError};

pub fn devkit_write_file(
    principal: &Principal,
    ws: &str,
    root: Option<&Path>,
    path: &Path,
    content: &str,
) -> Result<lb_devkit::WriteFileReport, DevkitError> {
    authorize_devkit(principal, ws, "write_file")?;
    lb_devkit::write_file(root, path, content)
        .map_err(|e| DevkitError::Devkit(e.to_string()))
}
