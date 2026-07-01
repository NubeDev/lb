//! `lb call <tool> '{json}'` — the universal escape hatch (operator-cli scope, Goals). Reaches EVERY
//! MCP verb through the one transport (`POST /mcp/call` remote, `call_tool` local) with zero per-verb
//! wiring, so the spine is provable in one command and any not-yet-wrapped verb is reachable. The
//! args are the operator's raw JSON, parsed defensively; the result is shaped per `-o`.

use serde_json::Value;

use crate::error::{CliError, CliResult};
use crate::output::Format;
use crate::transport::Transport;

use super::Printed;

/// Parse the JSON `args` string (default `{}`) and call `tool` through `transport`. A malformed args
/// string is a clean bad-input error, never a call with garbage. A server/host deny propagates as
/// `CliError::Denied` from the transport — this layer never fabricates a success.
pub async fn run(
    transport: &impl Transport,
    tool: &str,
    args_json: Option<&str>,
    format: Format,
) -> CliResult<Printed> {
    let args: Value = match args_json {
        None => Value::Object(Default::default()),
        Some(s) if s.trim().is_empty() => Value::Object(Default::default()),
        Some(s) => serde_json::from_str(s)
            .map_err(|e| CliError::BadInput(format!("args is not valid JSON: {e}")))?,
    };
    let header = transport.header();
    let result = transport.call(tool, args).await?;
    Printed::from_value(&header, &result, format)
}
