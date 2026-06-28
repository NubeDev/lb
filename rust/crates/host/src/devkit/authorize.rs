use lb_auth::Principal;
use lb_mcp::authorize_tool;

use super::DevkitError;

pub fn authorize_devkit(principal: &Principal, ws: &str, verb: &str) -> Result<(), DevkitError> {
    let tool = format!("devkit.{verb}");
    authorize_tool(principal, ws, &tool).map_err(|_| DevkitError::Denied)
}
