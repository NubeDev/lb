//! `lb reminder ls [--status enabled|disabled] [--limit N]` → `reminder.list`. The keyset-paged read
//! of the common grammar (the ws is the token's, never the body — the hard wall). `--status`/`--limit`
//! are the D3 shared-core filters, applied server-side by the extended `reminder.list` verb; the CLI
//! only forwards them. The result (`{reminders: [...]}`) is shaped DEFENSIVELY by the output layer — an
//! empty list reads as `(no rows)`, never an error and never an invented shape.

use serde_json::{json, Map, Value};

use crate::error::CliResult;
use crate::output::Format;
use crate::transport::Transport;

use crate::commands::Printed;

/// List reminders via `reminder.list`, forwarding the optional `status`/`limit` filters.
pub async fn run(
    transport: &impl Transport,
    status: Option<&str>,
    limit: Option<u32>,
    format: Format,
) -> CliResult<Printed> {
    let header = transport.header();
    let mut args = Map::new();
    if let Some(s) = status {
        args.insert("status".into(), json!(s));
    }
    if let Some(n) = limit {
        args.insert("limit".into(), json!(n));
    }
    let result = transport.call("reminder.list", Value::Object(args)).await?;
    Printed::from_value(&header, &result, format)
}
