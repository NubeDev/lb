//! The four **text ↔ structured** ops the `csv`/`xml`/`yaml`/`base64` built-in nodes call
//! (data-nodes scope, Risk 4). Each is a pure function of a `serde_json::Value` payload plus a small
//! `config` and a `mode` string — no store, no bus, no host seam. The host wires a node's configured
//! mode to one of these and swaps the returned value in as the new payload.
//!
//! The house contract, uniform across all four (and mirroring the `json` node's parity):
//! **malformed input FAILS the node** — a bad body surfaces as `Err(...)` instead of flowing a wrong
//! shape downstream. A `parse` mode insists the payload is a JSON *string* to decode; a
//! `stringify`/`encode` mode emits a JSON *string*. An unknown `mode` is always `Err`.
//!
//! Split by responsibility (FILE-LAYOUT): [`text`] owns `csv`/`yaml`/`base64` (line-oriented +
//! whole-value converters); [`xml`] owns the larger event-driven XML convention.

mod text;
mod xml;

pub use text::{base64, csv, yaml};
pub use xml::xml;

use serde_json::Value;

/// Render a scalar/absent value as a flat cell string: strings bare, everything else compact-JSON,
/// absent/null → empty. Shared by the CSV writer and the XML attribute/text writer.
pub(super) fn cell_string(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(other) => other.to_string(),
    }
}
