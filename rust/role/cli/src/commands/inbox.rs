//! `lb inbox list <channel>` — the ONE typed command (operator-cli scope v1 slice), the proof that
//! typed → `/mcp/call` shaping works. It maps to the `inbox.list` MCP verb (which takes `{channel}`
//! and returns `{ items: [...] }`) through the SAME transport as `lb call` — no new client path, no
//! typed REST route. The result is shaped DEFENSIVELY (the output layer unwraps the `{items}`
//! envelope) so an empty inbox reads as `(no rows)`, never as an error and never as an invented shape.

use serde_json::json;

use crate::error::CliResult;
use crate::output::Format;
use crate::transport::Transport;

use super::Printed;

/// List the inbox for `channel` via `inbox.list`. The channel is a required arg of the verb; the CLI
/// passes it through and shapes whatever the server returns (the drift-defense discipline).
pub async fn list(transport: &impl Transport, channel: &str, format: Format) -> CliResult<Printed> {
    let header = transport.header();
    let result = transport
        .call("inbox.list", json!({ "channel": channel }))
        .await?;
    Printed::from_value(&header, &result, format)
}
