// ChartTypeStep (panel-wizard scope, step 4) — the wizard's second step. Reuses the SHIPPED `VizPicker`
// (shape-validated, the same picker the editor's panel-options tab mounts), so the wizard offers exactly
// the views the editor does — no second viz surface, no drift. Picking a view writes it through `onChange`
// (which `PanelWizard.changeView` resolves into a `view` + per-view options reset, mirroring the editor's
// switch-viz behavior). One responsibility: pick a chart type.

import type { View } from "@/lib/dashboard";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { VizPicker } from "@/features/panel-builder/VizPicker";

interface Props {
  state: EditorState;
  onChange: (view: View) => void;
}

export function ChartTypeStep({ state, onChange }: Props) {
  const current = (state.view || "timeseries") as View;
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
    </div>
  );
}
