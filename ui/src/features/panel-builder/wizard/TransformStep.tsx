// TransformStep (panel-wizard scope, step 6) — the wizard's fourth step. A DATA step: transformations
// re-query (the backend `lb-viz`/`viz.query` runs the pipeline server-side), unlike the presentation
// options which only reshape cached frames. Reuses the editor's shipped `TransformTab` verbatim — same
// transform picker, same per-id typed editors, same move/disable/remove controls — so author and editor
// share ONE transform surface. Adds the freeze-current-data toggle (data-studio-ux) so an author can
// iterate against an expensive query without re-fetching on each transform edit.
//
// Every transform edit patches `state.transformations` (no wizard-only field); the full-panel preview
// re-fetches via `usePanelData`/`useVizQuery` (a data change re-fetches by design — invariant B). The
// freeze toggle, when on, pins the FETCH (the cell still re-SHAPES against its transforms over the
// frozen raw frames).
//
// One responsibility: the wizard's transform surface (a thin wrap over TransformTab + the freeze toggle).

import { Snowflake } from "lucide-react";

import type { Cell, View } from "@/lib/dashboard";
import { canonicalView } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { TransformTab } from "@/features/panel-builder/tabs/TransformTab";
import { Button } from "@/components/ui/button";

interface Props {
  state: EditorState;
  /** Apply a state patch (transform edits patch `transformations`). */
  patch: (next: Partial<EditorState>) => void;
  /** The serialized preview cell — forwarded to TransformTab's per-step debug view. */
  cell: Cell;
  /** Auto-refresh tick. */
  refreshKey?: number;
  /** The resolved variable scope. */
  scope?: VarScope;
  /** Frozen = the datasource is not re-hit on a transform edit; the cell reshapes cached raw frames. */
  frozen: boolean;
  /** Toggle the freeze (the "use current data" affordance). */
  onFrozenChange: (next: boolean) => void;
  /** Save the finished panel (step 8 — the trailing action of the wizard). */
  onSave: () => void;
  /** Is a save in flight? Disables the Save button + flips its label. */
  saving?: boolean;
}

export function TransformStep({
  state,
  patch,
  cell,
  refreshKey = 0,
  scope = emptyScope(),
  frozen,
  onFrozenChange,
  onSave,
  saving = false,
}: Props) {
  // Suppress the unused-view warning — the wizard's transform step is independent of view; the cell's
  // view is whatever ChartTypeStep picked.
  const _view = canonicalView(state.view || "timeseries") as View;
  void _view;

  return (
    <div className="grid gap-3" aria-label="wizard transform step">
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Transform (optional)</h2>
        <p className="text-xs text-muted">
          A data step — transformations re-query the source (the backend runs the pipeline).
        </p>
      </div>

      {/* The freeze-current-data toggle: pin the FETCH so iterating against an expensive query doesn't
          re-hit the datasource on each transform edit. The cell still re-SHAPES over the frozen frames. */}
      <label className="flex items-center gap-2 text-xs text-muted">
        <Button
          type="button"
          variant={frozen ? "default" : "outline"}
          size="sm"
          aria-pressed={frozen}
          aria-label="freeze current data"
          className="h-7"
          onClick={() => onFrozenChange(!frozen)}
        >
          <Snowflake size={12} className="mr-1" />
          {frozen ? "Frozen" : "Freeze current data"}
        </Button>
        <span>{frozen ? "transform edits reshape the cached frames — no re-fetch" : "transform edits re-fetch the source"}</span>
      </label>

      <TransformTab state={state} patch={patch} draft={cell} scope={scope} refreshKey={refreshKey} />

      <div className="mt-2 flex justify-end">
        <Button onClick={onSave} disabled={saving} aria-label="save panel">
          {saving ? "Saving…" : "Save panel"}
        </Button>
      </div>
    </div>
  );
}
