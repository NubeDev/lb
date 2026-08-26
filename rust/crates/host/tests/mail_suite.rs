//! Aggregated **mail-source** integration tests.
//!
//! One binary, files as modules — the same shape (and for the same reason) as `agent_suite.rs`:
//! Cargo compiles every top-level `tests/*.rs` as its own crate and statically links the whole
//! dependency graph (SurrealDB, Zenoh, wasmtime) into each one, ~1 GB apiece. Splitting a long test
//! file into two *targets* to satisfy the 400-line FILE-LAYOUT limit would pay that cost twice, and
//! add a second test process to an already heavily parallel suite. Declaring them as modules keeps
//! the file layout honest and produces a single binary.
//!
//!   - `mail/harness.rs`     — the real IMAP server, the real NEM12 export, the message builders.
//!   - `mail/import_test.rs` — what an arriving message BECOMES: assets, series, an inbox item —
//!                             and what it must never become twice.
//!   - `mail/source_test.rs` — who may point the platform at a mailbox at all: the capability deny,
//!                             the workspace wall, `check`, and re-registration.

#[path = "mail/harness.rs"]
mod harness;
#[path = "mail/import_test.rs"]
mod import_test;
#[path = "mail/source_test.rs"]
mod source_test;
