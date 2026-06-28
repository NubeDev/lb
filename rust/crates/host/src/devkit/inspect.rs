use std::path::Path;

use lb_auth::Principal;

use super::{authorize_devkit, DevkitError};

pub fn devkit_inspect(
    principal: &Principal,
    ws: &str,
    path: &Path,
) -> Result<lb_devkit::InspectReport, DevkitError> {
    authorize_devkit(principal, ws, "inspect")?;
    let root = lb_devkit::default_devkit_root();
    let safe = lb_devkit::resolve_under_root(root, path)
        .map_err(|e| DevkitError::BadInput(e.to_string()))?;
    lb_devkit::inspect_extension(&safe).map_err(|e| DevkitError::Devkit(e.to_string()))
}
