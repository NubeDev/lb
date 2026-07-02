//! `lb-cli` — the operator CLI client library (operator-cli scope). The load-bearing pieces —
//! transport, config, output shaping, the `ws/user/role` header — live HERE and are tested in-process
//! against a REAL gateway; `main.rs` is a thin shell that parses args and prints what these return.
//!
//! The whole surface is a **pure client** of the one `lb_host::call_tool` chokepoint: remote via
//! `POST /mcp/call`, local in-process. Zero new MCP verbs, capabilities, tables, or enforcement paths.
//! A command is denied exactly when the server (or the local host) denies — the CLI relays the
//! decision, it never fabricates a success.

pub mod cli;
pub mod commands;
pub mod config;
pub mod context;
pub mod dispatch;
pub mod error;
pub mod header;
pub mod login;
pub mod output;
pub mod sign;
pub mod transport;

pub use commands::Printed;
pub use dispatch::run;
pub use error::{CliError, CliResult};
