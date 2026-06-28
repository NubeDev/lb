use std::path::PathBuf;
use std::sync::Arc;

use lb_auth::Principal;
use lb_mcp::ToolError;
use serde_json::{json, Value};

use crate::Node;

use super::{devkit_build, devkit_inspect, devkit_scaffold, devkit_templates, DevkitError};

pub async fn call_devkit_tool(
    node: &Arc<Node>,
    principal: &Principal,
    ws: &str,
    qualified_tool: &str,
    input: &Value,
) -> Result<Value, ToolError> {
    match qualified_tool {
        "devkit.templates" => Ok(serde_json::to_value(devkit_templates(principal, ws)?).unwrap()),
        "devkit.scaffold" => {
            let req: lb_devkit::ScaffoldRequest = serde_json::from_value(input.clone())
                .map_err(|e| ToolError::BadInput(e.to_string()))?;
            let report = devkit_scaffold(principal, ws, None, &req)?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "devkit.inspect" => {
            let path = path_arg(input)?;
            let report = devkit_inspect(principal, ws, &path)?;
            Ok(serde_json::to_value(report).unwrap())
        }
        "devkit.build" => {
            let path = path_arg(input)?;
            let ts = input.get("ts").and_then(|v| v.as_u64()).unwrap_or(0);
            let started = devkit_build(node, principal, ws, &path, ts).await?;
            Ok(json!(started))
        }
        _ => Err(ToolError::NotFound),
    }
}

fn path_arg(input: &Value) -> Result<PathBuf, ToolError> {
    input
        .get("path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .ok_or_else(|| ToolError::BadInput("missing/invalid arg: path".into()))
}

impl From<DevkitError> for ToolError {
    fn from(value: DevkitError) -> Self {
        match value {
            DevkitError::Denied => ToolError::Denied,
            DevkitError::BadInput(m) => ToolError::BadInput(m),
            DevkitError::Devkit(m) | DevkitError::Bus(m) => ToolError::Extension(m),
            DevkitError::Store(e) => ToolError::Extension(e.to_string()),
        }
    }
}
