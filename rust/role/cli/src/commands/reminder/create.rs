//! `lb reminder create --channel <c> --body <t> --cron <expr>` → `reminder.create`. Friendly typed
//! sugar: the operator gives a channel/body/schedule, the CLI builds the channel-post action (the v1
//! default action kind), supplies the client-chosen `id` the verb requires (derived from the body
//! unless `--id` is given — a nice no-id UX), and stamps `ts` with the wall clock.
//!
//! Per resource-verbs D4 the command PRINTS THE ID, not the full record — `lb reminder show <id>` if
//! you want the record. The verb returns the whole reminder; the CLI extracts `id` and prints just
//! that (the `-o json` form still emits the raw envelope so a script can read every field).

use serde_json::{json, Value};

use crate::error::{CliError, CliResult};
use crate::output::Format;
use crate::transport::Transport;

use crate::commands::reminder::{derive_id, now_ts};
use crate::commands::Printed;

/// Create a channel-post reminder via `reminder.create`, printing the new id (D4).
#[allow(clippy::too_many_arguments)]
pub async fn run(
    transport: &impl Transport,
    channel: &str,
    body: &str,
    cron: &str,
    id: Option<&str>,
    max_runs: Option<u32>,
    format: Format,
) -> CliResult<Printed> {
    let header = transport.header();
    let now = now_ts();
    let id = id
        .map(|s| s.to_string())
        .unwrap_or_else(|| derive_id(body, now));

    let mut args = json!({
        "id": id,
        "schedule": cron,
        "action": { "kind": "channel-post", "channel": channel, "body": body },
        "ts": now,
    });
    if let Some(n) = max_runs {
        args["max_runs"] = json!(n);
    }

    let result = transport.call("reminder.create", args).await?;
    // D4: print the id, not the record. JSON callers get the raw envelope; the human form is the id
    // (read back from the server's response, so it is the id the store actually holds).
    match format {
        Format::Json => Printed::from_value(&header, &result, format),
        Format::Table => {
            let created = result
                .get("id")
                .and_then(Value::as_str)
                .ok_or_else(|| CliError::Other(format!("create returned no id: {result}")))?;
            Ok(Printed::new(&header, created))
        }
    }
}
