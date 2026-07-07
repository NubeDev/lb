// useWizardPreview (panel-wizard scope, step 5) — the wizard's named seam over the preview's data path.
// It serializes the wizard's `EditorState` to a preview cell + exposes a `bump()` to force a re-query
// (used by data steps — source/chart-type/transform — that DO re-fetch). The "presentation toggles
// don't re-query" cost model is delivered by the shipped `useVizQuery` fetch/shape split: a fieldConfig
// (presentation) edit re-keys ONLY the shape pass (`vizShapeKey = {framesHash, transformations,
// fieldConfig}`), never the fetch key (`vizFetchKey = {sources, source, scope, tick}`). This hook makes
// that contract addressable — the cost-model test counts `viz.query` calls and asserts:
//   - a presentation-option toggle (decimals) does NOT fire a second fetch;
//   - a data change (transform/chart-type/source) DOES.
//
// One responsibility: derive the wizard's preview cell + the manual refresh tick.

import { useCallback, useMemo, useState } from "react";

import type { View } from "@/lib/dashboard";
import { defaultCell } from "@/lib/panel-kit/defaultCell";
import { editorStateToCell, type EditorState } from "@/lib/panel-kit/cellEditorState";

/** The wizard's draft cell key — fresh per mount; the host assigns the real `i` at save. */
export const WIZARD_CELL_I = "wizard-draft";

export interface WizardPreview {
  /** The serialized preview cell — what `save` would persist. Re-derived on each state patch so the
   *  preview + every `OptionSectionCard` reflect the latest option. */
  cell: ReturnType<typeof editorStateToCell>;
  /** Bumps to force `usePanelData` to re-query (the data-step "Run" signal). */
  refreshKey: number;
  /** Force a re-query (used by data-step edits — source/chart-type/transform). */
  bump: () => void;
}

/** Derive the wizard's preview cell + refresh tick from its `EditorState`. */
export function useWizardPreview(state: EditorState): WizardPreview {
  const [refreshKey, setRefreshKey] = useState(0);
  const view = (state.view || "timeseries") as View;
  const cell = useMemo(
    () => editorStateToCell(state, defaultCell(view, WIZARD_CELL_I)),
    [state, view],
  );
  const bump = useCallback(() => setRefreshKey((k) => k + 1), []);
  return { cell, refreshKey, bump };
}
