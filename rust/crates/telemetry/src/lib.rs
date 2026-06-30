//! `lb-telemetry` — the **emission** half of observability (the minimal slice the telemetry console
//! depends on) + the capped SurrealDB sink. One span/event vocabulary emitted everywhere, and the
//! `tracing-subscriber` **layers** that select the sink by **config** (stderr / the capped ring /
//! OTLP peer) — never a code branch on role (symmetric nodes, rule #1).
//!
//! Verbs, one responsibility per file (FILE-LAYOUT §3):
//! - [`secret`] — `Secret<T>`: the redaction *type* (Debug/Display → `***`), so a secret can never
//!   be accidentally formatted into an event.
//! - [`redact`] — `params_digest`: a SHA-256 + shape summary of tool params (the helper shared with
//!   audit). Raw params NEVER reach an event — only the digest.
//! - [`record`] — the one event schema (`TelemetryRecord`) + the stored field shapes.
//! - [`emit`] — `record_dispatch`: emit the redacted dispatch event through `tracing`.
//! - [`layer`] — `SurrealCappedLayer`: the subscriber layer that writes each event to the FIFO-capped
//!   `telemetry` table via [`lb_store::capped_insert`], peer to the OTLP/stderr layers.
//! - [`config`] — the layer set the `node` binary wires from env (config-selected sink).
//!
//! This crate owns the *operational* projection only (observability scope). The audit ledger and the
//! undo journal are sibling projections of the same dispatch chokepoint — different stores, different
//! guarantees; this sink is sampled and will evict (the scope says so, the console labels it).

mod config;
mod emit;
mod error;
mod layer;
mod record;
mod redact;
mod secret;
mod subject;

pub use config::{sink_layers, SinkConfig};
pub use emit::{record_dispatch, TARGET};
pub use error::TelemetryError;
pub use layer::{
    collect_event, write_event, KeySelector, SurrealCappedLayer, DEFAULT_CAP, DEFAULT_SAMPLE_EVERY,
};
pub use record::{Level, Outcome, TelemetryRecord, TABLE};
pub use redact::params_digest;
pub use secret::Secret;
pub use subject::TAIL_SUBJECT;
