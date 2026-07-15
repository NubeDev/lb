// The undo journal's wire shapes (undo-exposure scope) — what `GET /undo/history`, `POST /undo`,
// and `POST /redo` actually return. Mirrors the host's typed outcomes: a refusal is DATA
// (`{ok:false, reason}`), not an error, so the shell can grey a control or explain a stale step
// without treating it as a failure. Only a capability deny throws (`InvokeError.isDenied`).

/** Why an undo/redo could not apply right now — the shell renders each differently. */
export type UndoRefusal =
  /** Nothing left on this stack (grey the control). */
  | "empty"
  /** The record changed since the step: the undo is refused, never a clobber (explain it). */
  | "stale"
  /** An irreversible step — offer its compensation instead, if it has one. */
  | "not_undoable";

/** The result of `undo` / `redo`: applied, or a typed refusal. */
export type UndoOutcome =
  | { ok: true; seq?: number }
  | { ok: false; reason: UndoRefusal; seq?: number };

/** One row of the undo history, newest-first. */
export interface UndoHistoryItem {
  seq: number;
  /** The tool that produced the step (`doc.rename`) — the row's label. */
  tool: string;
  /** False for an irreversible/compensable step: the row is greyed, not actionable. */
  undoable: boolean;
  /** True once the step has been undone — it sits on the redo side. */
  redoable: boolean;
  ts: number;
}

/** `GET /undo/history` — the caller's own stack. */
export interface UndoHistory {
  items: UndoHistoryItem[];
}

/** `GET /undo/history/{seq}/compensations` — `null` when the step offers no compensation. */
export interface UndoCompensations {
  compensation_tool: string | null;
}
