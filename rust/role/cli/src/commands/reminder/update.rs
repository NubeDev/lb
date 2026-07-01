//! `lb reminder update <id> [--enabled true|false] [--cron <expr>] [--max-runs N]` →
//! `reminder.update`. The grammar's `update` covers pause/resume (`--enabled`) and reschedule
//! (`--cron`) via one verb — no bespoke pause/resume command. Only the flags the operator passed are
//! sent (a partial patch); `ts` is stamped with the wall clock. The verb returns the updated record,
//! which the CLI shapes like any read.

use serde_json::{json, Value};

use crate::error::CliResult;
use crate::output::Format;
use crate::transport::Transport;

use crate::commands::reminder::now_ts;
use crate::commands::Printed;

/// Update reminder `id` via `reminder.update`, sending only the fields the operator changed.
pub async fn run(
    transport: &impl Transport,
    id: &str,
    enabled: Option<bool>,
    cron: Option<&str>,
    max_runs: Option<u32>,
    format: Format,
) -> CliResult<Printed> {
    let header = transport.header();
    let mut args = json!({ "id": id, "ts": now_ts() });
    if let Some(e) = enabled {
        args["enabled"] = json!(e);
    }
    if let Some(c) = cron {
        args["schedule"] = json!(c);
    }
    if let Some(n) = max_runs {
        args["max_runs"] = json!(n);
    }
    let result: Value = transport.call("reminder.update", args).await?;
    Printed::from_value(&header, &result, format)
}
