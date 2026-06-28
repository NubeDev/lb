//! Auto-capture-on-dispatch: journal every mutating tool call into the undo journal automatically,
//! classified from **runtime outbox taint** (`docs/scope/undo/undo-scope.md`). The manual
//! `lb_undo` mechanism + the host `undo`/`redo` verbs ship separately; this is the wiring that makes
//! capture happen for free at the dispatch chokepoint, so no extension calls `record_change` itself.
//!
//! Two files (FILE-LAYOUT §3):
//!   - `plan` — classify a call: capturable single-record reversible | non-generic | not-mutating.
//!   - `capture` — wrap dispatch in a taint scope and journal the outcome (taint wins over the plan).

mod capture;
mod plan;

pub(crate) use capture::capture_dispatch;
