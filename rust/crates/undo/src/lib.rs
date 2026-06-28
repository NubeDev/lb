//! The undo journal — a reversible-command journal at the host (README §6.5/§6.8,
//! `docs/scope/undo/undo-scope.md`).
//!
//! **Reverse state, compensate motion.** A reversible state mutation (a SurrealDB record change) is
//! undoable by restoring its before-image; irreversible *motion* (an outbox effect) is never undone
//! — it is *compensated*. The classification is derived from runtime taint (`classify`), never
//! trusted from a manifest, and a mixed action is irreversible as a whole (the `max` composition).
//!
//! The correctness core is the **conditional restore** (`restore`): an undo applies only if every
//! touched record's current `rev` still matches what the step expects — enforced in one transaction,
//! so a stale undo is *refused*, never a forced last-writer-wins clobber. This makes undo safe across
//! sync: the predicate travels with the operation and is enforced where it actually applies.
//!
//! Two record shapes (the scope's "immutable events plus a materialized cursor"): immutable
//! [`JournalEntry`] events, and a mutable [`StackState`] cursor. Verbs, one per file (FILE-LAYOUT §3):
//!   - [`record_change`] — a reversible `do`: capture before-image + apply, atomically.
//!   - [`record_irreversible`] — a not-undoable marker (irreversible/compensable).
//!   - [`apply_undo`] / [`apply_redo`] — the conditional restore + cursor move.
//!   - [`history::list`] / [`history::compensations`] — the read side, for a UI.
//!   - [`classify`] — the runtime-taint → [`Class`] rule.
//!
//! Raw verbs — capability checks + the workspace wall are the host layer's job (`lb-host`).

mod apply_redo;
mod apply_undo;
mod classify;
mod error;
mod history;
mod model;
mod peek;
mod persist;
mod record_captured;
mod record_change;
mod record_irreversible;
mod restore;

pub use apply_redo::apply_redo;
pub use apply_undo::apply_undo;
pub use classify::classify;
pub use error::UndoError;
pub use history::{compensations, list, HistoryItem};
pub use model::{Class, JournalEntry, Kind, StackState, TouchedRecord, DEFAULT_DEPTH_CAP};
pub use peek::{peek_redo, peek_undo};
pub use record_captured::{record_captured, RecordCaptured};
pub use record_change::{record_change, RecordChange};
pub use record_irreversible::{record_irreversible, RecordIrreversible};
