// The `piechart` per-viz options editor — registry-driven (editor-parity step 5): pieType + legend show
// from the registry; displayLabels (a `PieChartLabel[]` array, not a scalar) keeps its bespoke
// multi-toggle; reduceOptions calc as a bespoke extra. One responsibility: compose the pie per-viz opts.

import { Button } from "@/components/ui/button";
import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { readPieChartOptions, type PieChartLabel } from "@/features/dashboard/views/piechart/options";
import { VizOptions } from "./VizOptions";
import { ReduceOptionsEditor } from "./ReduceOptionsEditor";

const LABELS: PieChartLabel[] = ["name", "value", "percent"];

export function PieChartOptionsEditor({ state, patch }: { state: EditorState; patch: (n: Partial<EditorState>) => void }) {
  const o = readPieChartOptions(state.options);
  const set = (next: Partial<typeof o>) => patch({ options: { ...state.options, ...next } });
  const toggleLabel = (l: PieChartLabel) => {
    const has = o.displayLabels.includes(l);
    set({ displayLabels: has ? o.displayLabels.filter((x) => x !== l) : [...o.displayLabels, l] });
  };
  return (
    <VizOptions
      view="piechart"
      state={state}
      patch={patch}
      extras={
        <div className="grid gap-2 text-xs">
          <div className="grid gap-1 text-muted">
            Display labels
            <div className="flex flex-wrap gap-1.5">
              {LABELS.map((l) => (
                <Button
                  key={l}
                  variant={o.displayLabels.includes(l) ? "default" : "outline"}
                  size="sm"
                  aria-label={`pie label ${l}`}
                  aria-pressed={o.displayLabels.includes(l)}
                  className="h-auto px-2 py-0.5 text-[11px]"
                  onClick={() => toggleLabel(l)}
                >
                  {l}
                </Button>
              ))}
            </div>
          </div>
          <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => set({ reduceOptions })} />
        </div>
      }
    />
  );
}
