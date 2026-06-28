//! The canonical **run-event vocabulary** (agent-run scope Part 1) — one typed `RunEvent` enum,
//! **derived from** the durable job transcript (event-sourced), so a live stream and a
//! reconnect/`session/load` replay are projections of the same record and never diverge.
//!
//! This crate sits in the lowest layer on purpose: it depends only on `lb-jobs` (the transcript it
//! projects) and serde — **no** deps on protocols, the gateway, or wasm. Every external wire format
//! (the gateway SSE route, the ACP adapter, a future AI-SDK encoder) is a thin `RunEvent -> wire`
//! function in its *own* role crate, importing this. That keeps the loop ignorant of the word "ACP"
//! and makes a second encoder purely additive (rule 1: one model, many projections — no parallel
//! path).
//!
//! - [`RunEvent`] / [`RunOutcome`] — the vocabulary.
//! - [`project`] — a whole job → the snapshot sequence (a late watcher's catch-up).
//! - [`project_one`] — one transcript event → its live run events (what the loop emits as it goes).

mod event;
mod project;

pub use event::{RunEvent, RunOutcome};
pub use project::{project, project_one, terminal_outcome};
