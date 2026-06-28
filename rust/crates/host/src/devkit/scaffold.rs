use std::path::Path;

use lb_auth::Principal;

use super::{authorize_devkit, DevkitError};

pub fn devkit_scaffold(
    principal: &Principal,
    ws: &str,
    root: Option<&Path>,
    request: &lb_devkit::ScaffoldRequest,
) -> Result<lb_devkit::ScaffoldReport, DevkitError> {
    authorize_devkit(principal, ws, "scaffold")?;
    lb_devkit::scaffold_extension(root, request).map_err(|e| DevkitError::Devkit(e.to_string()))
}
