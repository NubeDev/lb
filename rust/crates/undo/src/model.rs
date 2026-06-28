//! The undo journal's record types (`docs/scope/undo/undo-scope.md`).
//!
//! Two shapes, not one (the scope's "immutable events plus a materialized cursor"):
//!   - [`JournalEntry`] — an **immutable** event row (`do`/`undo`/`redo`), each carrying the
//!     before/after images and the per-record `rev` the conditional restore tests against. These
//!     sync append-style like audit rows.
//!   - [`StackState`] — the **mutable** per-(ws, actor[, surface]) cursor: where undo/redo point
//!     in the entry sequence. An ordinary LWW state record.
//!
//! Class is the reversibility classification. The authoritative value is **derived from runtime
//! taint** (did the transaction reach the outbox?), not trusted from a manifest — see
//! `classify.rs`. A manifest may only *add* a [`Class::Compensable`] handle to a derived
//! [`Class::Irreversible`]; it can never downgrade one.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// What a journal entry records about the action that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    /// A forward action (the original `do`).
    Do,
    /// An undo of a prior `Do`/`Redo` (restores its before-image).
    Undo,
    /// A redo of a prior `Undo` (re-applies its after-image).
    Redo,
}

/// The reversibility classification of an action (the load-bearing boundary, scope "Goals" #2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Class {
    /// A pure state mutation — fully undoable by restoring its before-image.
    Reversible,
    /// The transaction reached the outbox (external motion). **Never undoable**; surfaced greyed.
    Irreversible,
    /// Irreversible, but the action declared a compensating tool to offer instead of an undo.
    /// Holds the compensating tool name (the "declare a handle" shape; the orchestrator is
    /// deferred to jobs).
    Compensable { compensation_tool: String },
}

impl Class {
    /// True if an entry of this class can be reversed by restoring its before-image. Only
    /// [`Class::Reversible`] is undoable; irreversible/compensable are not (the latter offers a
    /// compensation instead).
    pub fn is_undoable(&self) -> bool {
        matches!(self, Class::Reversible)
    }

    /// The composition rule: the class of an action is the **max** over its parts. Reversible is
    /// the floor; any irreversible/compensable part dominates. A compensable part beats a plain
    /// irreversible one only by *carrying* a compensation — it is still not undoable.
    pub fn combine(self, other: Class) -> Class {
        match (self, other) {
            // Any compensable wins (it is irreversible *with* a handle); keep the first handle seen.
            (c @ Class::Compensable { .. }, _) => c,
            (_, c @ Class::Compensable { .. }) => c,
            (Class::Irreversible, _) | (_, Class::Irreversible) => Class::Irreversible,
            (Class::Reversible, Class::Reversible) => Class::Reversible,
        }
    }
}

/// The expected revision of one touched record — half of the conditional-restore predicate. Undo
/// applies only if the record's *current* `rev` still equals `expected_after_rev` (no intervening
/// writer); the restore then writes `before` back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TouchedRecord {
    pub table: String,
    pub id: String,
    /// The record's state before this action (the undo target). `None` = the record was absent
    /// (a *create* — undo deletes it back to absence).
    pub before: Option<Value>,
    /// The record's state after this action (the redo target). `None` = the record became absent
    /// (a *delete*).
    pub after: Option<Value>,
    /// The `rev` the action produced — what the current record must still equal for undo to apply.
    /// [`lb_store::Versioned::ABSENT_REV`] (0) when `after` is `None` (still-absent predicate).
    pub expected_after_rev: u64,
    /// The `rev` the *before* state had — what the current record must equal for **redo** to apply
    /// after an undo (undo restores `before`, so redo's predicate is the before-rev).
    pub expected_before_rev: u64,
}

/// One immutable journal event. The undo stack is the sequence of these per (ws, actor[, surface]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalEntry {
    /// Monotonic per-(ws) sequence number — the entry's id is `undo:{seq}`.
    pub seq: u64,
    pub ws: String,
    pub actor: String,
    /// Optional finer stack key (per-document, per-session) for editor-style undo. Empty = the
    /// default per-(ws, actor) stack.
    #[serde(default)]
    pub surface: String,
    /// The tool whose call this entry records (e.g. `doc.rename`) — undo requires its cap.
    pub tool: String,
    pub kind: Kind,
    pub class: Class,
    /// The records this action touched (before/after/rev). A single-record action has one; a
    /// grouped action (a job/batch) has many and is undone all-or-nothing.
    pub touched: Vec<TouchedRecord>,
    /// Group id for multi-step actions (a job/import). Entries sharing a `group` undo together, in
    /// reverse order, all-or-nothing. A standalone action's group is its own `undo:{seq}` id.
    pub group: String,
    /// Caller-injected logical timestamp + trace id for audit correlation.
    pub trace_id: String,
    pub ts: u64,
}

/// The mutable cursor for one (ws, actor[, surface]) stack — id `undo_stack:{actor}` or
/// `undo_stack:{actor}:{surface}`. Holds which entries are live (undoable) vs already-undone
/// (redoable). The cursor is an ordinary LWW state record; the entries it points at are immutable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackState {
    pub ws: String,
    pub actor: String,
    #[serde(default)]
    pub surface: String,
    /// `seq`s available to **undo**, oldest→newest. The newest is the next undo target.
    pub undoable: Vec<u64>,
    /// `seq`s available to **redo**, oldest→newest. The newest is the next redo target. A new `do`
    /// truncates (clears) this.
    pub redoable: Vec<u64>,
}

impl StackState {
    pub fn new(
        ws: impl Into<String>,
        actor: impl Into<String>,
        surface: impl Into<String>,
    ) -> Self {
        Self {
            ws: ws.into(),
            actor: actor.into(),
            surface: surface.into(),
            undoable: Vec::new(),
            redoable: Vec::new(),
        }
    }

    /// Record a fresh forward `do`: it becomes the newest undoable step and **truncates the redo
    /// stack** (standard semantics — new work invalidates the redo future).
    pub fn push_do(&mut self, seq: u64, depth_cap: usize) {
        self.undoable.push(seq);
        self.redoable.clear();
        // Bounded depth: drop the oldest undoable beyond the cap (it becomes un-undoable; the
        // immutable entry is pruned separately).
        while self.undoable.len() > depth_cap {
            self.undoable.remove(0);
        }
    }

    /// The next step an undo would target (newest undoable), without popping.
    pub fn peek_undo(&self) -> Option<u64> {
        self.undoable.last().copied()
    }

    /// The next step a redo would target (newest redoable), without popping.
    pub fn peek_redo(&self) -> Option<u64> {
        self.redoable.last().copied()
    }

    /// Pop the undo target onto the redo stack (called after a successful undo).
    pub fn commit_undo(&mut self) -> Option<u64> {
        let seq = self.undoable.pop()?;
        self.redoable.push(seq);
        Some(seq)
    }

    /// Pop the redo target back onto the undo stack (called after a successful redo).
    pub fn commit_redo(&mut self) -> Option<u64> {
        let seq = self.redoable.pop()?;
        self.undoable.push(seq);
        Some(seq)
    }
}

/// Default bounded depth of an undo stack (scope: "bounded depth", config later).
pub const DEFAULT_DEPTH_CAP: usize = 100;

/// The journal-entry table within a workspace namespace. `undo:{seq}`.
pub(crate) const ENTRY_TABLE: &str = "undo";
/// The stack-state table within a workspace namespace. `undo_stack:{actor}[:{surface}]`.
pub(crate) const STACK_TABLE: &str = "undo_stack";
/// The per-(ws) sequence counter record: `undo_seq:counter`.
pub(crate) const SEQ_TABLE: &str = "undo_seq";
pub(crate) const SEQ_ID: &str = "counter";
