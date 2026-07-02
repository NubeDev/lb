//! `lb reminder show <id>` → `reminder.get`. One record by id (the common grammar's `get`). The verb
//! returns `{reminder: <record|null>}`; the CLI unwraps that envelope so the table is a `field | value`
//! view of the record itself (and a missing/tombstoned id reads as `(no rows)`), never a one-row
//! "reminder" cell. `-o json` still round-trips the raw envelope verbatim.

use serde_json::{json, Value};

use crate::error::CliResult;
use crate::output::Format;
use crate::transport::Transport;

use crate::commands::Printed;

/// Get reminder `id` via `reminder.get`, unwrapping the `{reminder}` envelope for the table view.
pub async fn run(transport: &impl Transport, id: &str, format: Format) -> CliResult<Printed> {
    let header = transport.header();
    let result = transport.call("reminder.get", json!({ "id": id })).await?;
    // JSON output is the raw envelope (drift-defense: never reshape what the server sent). The table
    // unwraps to the record so the operator reads its fields, and a null (absent/tombstoned) shows as
    // an empty result rather than a literal "null" cell.
    let shaped = match format {
        Format::Json => result,
        Format::Table => match result.get("reminder") {
            Some(Value::Null) | None => json!([]),
            Some(inner) => inner.clone(),
        },
    };
    Printed::from_value(&header, &shaped, format)
}
