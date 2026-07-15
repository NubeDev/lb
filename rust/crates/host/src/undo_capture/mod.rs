//! Auto-capture-on-dispatch: journal every mutating tool call into the undo journal automatically,
//! classified from **runtime outbox taint** (`docs/scope/undo/undo-scope.md`). The manual
//! `lb_undo` mechanism + the host `undo`/`redo` verbs ship separately; this is the wiring that makes
//! capture happen for free at the dispatch chokepoint, so no extension calls `record_change` itself.
//!
//! Three files (FILE-LAYOUT §3):
//!   - `plan` — classify a call: capturable single-record reversible | non-generic | not-mutating.
//!   - `decide` — the PURE outcome mapping (taint wins; a failed before-read is not-undoable,
//!     never "absent") — unit-tested so the distinction can't silently regress.
//!   - `capture` — wrap dispatch in a taint scope and journal the decided outcome.

mod capture;
mod decide;
mod plan;

pub(crate) use capture::capture_dispatch;
