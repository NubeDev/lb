//! `devkit.root` — the absolute devkit root directory that `inspect`/`build`/`publish` resolve every
//! path under. The Studio's "open existing" folder picker starts here: paths outside the root are
//! rejected by `resolve_under_root` anyway, so the picker never needs (or offers) anywhere else.

use lb_auth::Principal;
use serde::Serialize;

use super::{authorize_devkit, DevkitError};

#[derive(Debug, Clone, Serialize)]
pub struct DevkitRoot {
    pub path: String,
    pub os: String,
}

pub fn devkit_root(principal: &Principal, ws: &str) -> Result<DevkitRoot, DevkitError> {
    authorize_devkit(principal, ws, "root")?;
    // Match what the other verbs resolve against: create the root if missing, then canonicalize so the
    // UI gets a stable absolute anchor to browse from (and to build child paths that pass the gate).
    let root = lb_devkit::default_devkit_root();
    let _ = std::fs::create_dir_all(&root);
    let abs = root.canonicalize().unwrap_or(root);
    Ok(DevkitRoot {
        path: abs.to_string_lossy().replace('\\', "/"),
        os: std::env::consts::OS.to_string(),
    })
}
