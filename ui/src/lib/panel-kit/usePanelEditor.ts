// The headless panel-editor state machine (panel-kit; data-studio scope v2). This is the LOGIC the old
// modal `PanelEditor` chrome trapped: the working `EditorState` seeded from a cell through the ONE
// (de)serializer, patching, the viz switch (reset per-view options, preserve targets/fieldConfig/
// transformations), the serialized draft cell, and the preview refresh/run tick. NO JSX, NO data
// fetching, NO shell imports — a view composes this with `usePanelData`/`WidgetHost` (the substrate)
// and whatever chrome it wants (the dashboard modal is gone; Data Studio mounts it in FlexLayout panes;
// a third consumer can bring 100% different views).

import { useMemo, useState } from "react";

import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";

import { cellToEditorState, editorStateToCell, type EditorState } from "./cellEditorState";

/** The cartesian chart views that support the shared X/Y plot builder (the Plot tab). */
export const PLOTTABLE_VIEWS: ReadonlySet<string> = new Set(["timeseries", "barchart", "piechart"]);

/** Views that bind NO data source — they read their own data (e.g. `insights` reads the `insight.*`
 *  verbs through its own client), so the wizard's Source step is optional for them and the "pick a
 *  source" gate must not block advancing. A source-bound view (every viz, genui, template) is absent. */
export const SOURCELESS_VIEWS: ReadonlySet<string> = new Set(["insights"]);

export interface PanelEditorMachine {
  /** The full working state — every authorable group, rebuilt from the cell via the ONE serializer. */
  state: EditorState;
  /** Merge a partial update into the working state. */
  patch: (next: Partial<EditorState>) => void;
  /** The view canonicalized for DISPLAY (picker highlight / per-view tab branching); the stored
   *  `state.view` stays raw so a v2 `chart` cell serializes byte-identical. */
  viewC: View;
  /** `state` with the canonicalized view — what per-view option tabs read. */
  stateC: EditorState;
  /** Switch the viz: per-view `options` reset to the injected defaults; targets/sql/fieldConfig/
   *  transformations/title carry over (the "viz switch must preserve compatible state" rule). */
  switchView: (view: View) => void;
  /** The draft cell = what save would persist (also the preview's input). Same serializer as save. */
  draft: Cell;
  /** Serialize the current state against the base cell (what `onSave` should persist). */
  toCell: () => Cell;
  /** A cheap change/run tick — feed to `usePanelData` so the preview re-queries on source edits and
   *  on an explicit Run. */
  refreshKey: number;
  /** Force a preview re-query even when the spec is byte-identical (the Query tab's Run button). */
  run: () => void;
  /** A Flows binding swaps the offered viz set: INPUT (`flows.inject` action) / OUTPUT
   *  (`flows.node_state` read) / null (the standard set). */
  flowKind: "input" | "output" | null;
  /** Whether the current view supports the shared X/Y plot builder. */
  canPlot: boolean;
}

export interface UsePanelEditorOptions {
  /** The per-view Grafana default option block — INJECTED (the view substrate's registry owns it;
   *  panel-kit stays headless of the views). */
  defaultOptionsForView: (view: View) => Record<string, unknown>;
}

/** The ONE editor state machine, ADD and EDIT alike — seed with `defaultCell(...)` or a saved cell.
 *  Re-seeds itself when the cell IDENTITY (`cell.i`) changes (a new edit target in the same mount). */
export function usePanelEditor(cell: Cell, opts: UsePanelEditorOptions): PanelEditorMachine {
  const [state, setState] = useState<EditorState>(() => cellToEditorState(cell));
  const [seededFor, setSeededFor] = useState(cell.i);
  if (seededFor !== cell.i) {
    setState(cellToEditorState(cell));
    setSeededFor(cell.i);
  }

  // An explicit "Run" nonce folded into the refresh tick: re-fires the preview query even when the
  // spec is byte-identical (re-running the same SQL).
  const [runNonce, setRunNonce] = useState(0);
  const refreshKey = useMemo(
    () => JSON.stringify(state.targets).length + (state.sql?.rawSql.length ?? 0) + runNonce,
    [state.targets, state.sql, runNonce],
  );

  const patch = (next: Partial<EditorState>) => setState((s) => ({ ...s, ...next }));
  const viewC = canonicalView((state.view || "timeseries") as View);
  const stateC = { ...state, view: viewC };

  const switchView = (view: View) =>
    setState((s) => ({ ...s, view, options: opts.defaultOptionsForView(view) }));

  const flowKind: "input" | "output" | null =
    state.carry.action?.tool === "flows.inject"
      ? "input"
      : state.targets[0]?.tool === "flows.node_state"
        ? "output"
        : null;

  const draft = useMemo(() => editorStateToCell(state, cell), [state, cell]);

  return {
    state,
    patch,
    viewC,
    stateC,
    switchView,
    draft,
    toCell: () => editorStateToCell(state, cell),
    refreshKey,
    run: () => setRunNonce((n) => n + 1),
    flowKind,
    canPlot: PLOTTABLE_VIEWS.has(viewC),
  };
}
