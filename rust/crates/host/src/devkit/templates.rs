use lb_auth::Principal;

use super::{authorize_devkit, DevkitError};

pub fn devkit_templates(
    principal: &Principal,
    ws: &str,
) -> Result<Vec<lb_devkit::TemplateInfo>, DevkitError> {
    authorize_devkit(principal, ws, "templates")?;
    Ok(lb_devkit::templates())
}
