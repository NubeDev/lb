// The `piechart` per-viz options editor (viz panel-editor scope). Edits the typed `PieChartOptions` —
// reduceOptions + pieType + displayLabels + legend show. One responsibility: edit a piechart cell's
// `options`.

import { Button } from "@/components/ui/button";
import type { EditorState } from "../../cellEditorState";
import { readPieChartOptions, type PieChartType, type PieChartLabel } from "../../../views/piechart/options";
import { SelectField, ToggleField } from "./controls";
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
    <div className="grid gap-3 py-3 text-xs" aria-label="panel options tab">
      <ReduceOptionsEditor value={o.reduceOptions} onChange={(reduceOptions) => set({ reduceOptions })} />
      <SelectField<PieChartType>
        label="Pie type"
        value={o.pieType}
        options={["pie", "donut"]}
        onChange={(pieType) => set({ pieType })}
      />
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
      <ToggleField
        label="Show legend"
        checked={o.legend.showLegend}
        onChange={(showLegend) => set({ legend: { ...o.legend, showLegend } })}
      />
    </div>
  );
}
