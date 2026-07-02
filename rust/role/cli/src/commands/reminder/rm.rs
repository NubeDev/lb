//! `lb reminder rm <id> [--hard]` → `reminder.delete`. The grammar's `delete`: a soft tombstone (the
//! reminder never fires or lists again, kept only for audit). `--hard` is accepted for grammar
//! uniformity and forwarded as `hard:true`; the shipped verb currently treats delete as soft, so it is
//! a harmless passthrough today (the flag exists so the surface matches every other family's `rm`, and
//! becomes load-bearing when soft/undo lands per resource-verbs D2). `ts` is the wall clock.

use serde_json::json;

use crate::error::CliResult;
use crate::output::Format;
use crate::transport::Transport;

use crate::commands::reminder::now_ts;
use crate::commands::Printed;

/// Delete reminder `id` via `reminder.delete` (soft tombstone; `--hard` forwarded).
pub async fn run(
    transport: &impl Transport,
    id: &str,
    hard: bool,
    format: Format,
) -> CliResult<Printed> {
    let header = transport.header();
    let result = transport
        .call(
            "reminder.delete",
            json!({ "id": id, "ts": now_ts(), "hard": hard }),
        )
        .await?;
    Printed::from_value(&header, &result, format)
}
