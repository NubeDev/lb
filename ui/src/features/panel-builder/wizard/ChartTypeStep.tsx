// ChartTypeStep (panel-wizard scope, step 4) — the wizard's second step. Reuses the SHIPPED `VizPicker`
// (shape-validated, the same picker the editor's panel-options tab mounts), so the wizard offers exactly
// the views the editor does — no second viz surface, no drift. Picking a view writes it through `onChange`
// (which `PanelWizard.changeView` resolves into a `view` + per-view options reset, mirroring the editor's
// switch-viz behavior). For plottable views it also mounts the editor's SAME `PlotAxesTab` (the shared
// `PlotBuilder` — chart type + X/Y axes against the draft query's live fields), so the wizard configures
// the plot exactly where the chart type is chosen. One responsibility: pick a chart type (+ its plot).

import type { Cell, View } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { PLOTTABLE_VIEWS } from "@/lib/panel-kit";
import { VizPicker } from "@/features/panel-builder/VizPicker";
import { PlotAxesTab } from "@/features/panel-builder/tabs/PlotAxesTab";

interface Props {
  state: EditorState;
  onChange: (view: View) => void;
  /** The wizard's serialized draft cell — supplies the plot editor's query fields. */
  draft: Cell;
  /** Writes `options.plot` without a view reset (the plot edit path). */
  patch: (next: Partial<EditorState>) => void;
  refreshKey: number;
}

export function ChartTypeStep({ state, onChange, draft, patch, refreshKey }: Props) {
  const current = (state.view || "timeseries") as View;
  const canPlot = PLOTTABLE_VIEWS.has(current);
  return (
    <div className="grid gap-3" aria-label="wizard chart-type step">
      <div className="grid gap-1">
        <h2 className="text-sm font-medium text-fg">Pick a chart type</h2>
        <p className="text-xs text-muted">
          The picker disables views the current data shape can't honestly fill.
        </p>
      </div>
      <VizPicker view={current} onChange={onChange} />
      <p className="text-[11px] text-muted" aria-label="wizard view picked">
        current: <code className="text-fg">{current}</code>
      </p>
      {canPlot && (
        <div className="grid gap-1 border-t border-border pt-3">
          <h2 className="text-sm font-medium text-fg">Plot</h2>
          <PlotAxesTab draft={draft} state={state} patch={patch} refreshKey={refreshKey} />
        </div>
      )}
    </div>
  );
}
