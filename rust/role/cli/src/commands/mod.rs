//! The command layer — thin orchestrators that turn a resolved transport + format into printed
//! output. Each command is one file (FILE-LAYOUT: one verb per file). The shared piece is
//! [`Printed`]: every command's success is "a header line + a rendered body", so the header (which
//! makes the wall legible) is emitted uniformly and can never be forgotten by a command.
//!
//! Commands return the [`Printed`] value rather than printing directly, so they are unit-/integration-
//! testable (assert on the strings) and `main.rs` owns the single `println!`. A DENY never produces a
//! `Printed` — it is a `CliError` that `main` renders to stderr and a non-zero exit.

pub mod call;
pub mod devkit;
pub mod ext;
pub mod inbox;
pub mod login;
pub mod reminder;
pub mod whoami;

use crate::header::Header;
use crate::output::{self, Format};
use serde_json::Value;

use crate::error::CliResult;

/// A command's rendered output: the `ws/user/role` header line plus the body. Split so a caller (or a
/// test) can inspect each, and so `main` prints the header to stderr (context) and the body to stdout
/// (the data a pipe consumes) — keeping `-o json` output a clean, header-free stream on stdout.
#[derive(Debug, Clone)]
pub struct Printed {
    pub header: String,
    pub body: String,
}

impl Printed {
    /// Build from a header + an already-rendered body string.
    pub fn new(header: &Header, body: impl Into<String>) -> Self {
        Self {
            header: header.render(),
            body: body.into(),
        }
    }

    /// Build from a header + a tool result value, shaping the body per `format` (the common case).
    pub fn from_value(header: &Header, value: &Value, format: Format) -> CliResult<Self> {
        Ok(Self::new(header, output::render(value, format)?))
    }
}
