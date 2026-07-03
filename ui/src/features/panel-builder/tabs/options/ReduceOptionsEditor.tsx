// The shared `reduceOptions` editor (viz chart-types scope — the frame→value bridge for the single-stat
// family). stat/gauge/bargauge/piechart all edit `{calcs, values, limit}` the same way, so they share
// this. The calc list mirrors `reduce.ts#reduceCalc`. One responsibility: edit a `ReduceDataOptions`.

import { Button } from "@/components/ui/button";
import { type ReduceDataOptions, readReduceOptions } from "@/features/dashboard/views/reduce";
import { ToggleField } from "./controls";

const CALCS = ["lastNotNull", "last", "first", "mean", "max", "min", "sum", "count"];

export function ReduceOptionsEditor({
  value,
  onChange,
}: {
  value: unknown;
  onChange: (next: ReduceDataOptions) => void;
}) {
  const ro = readReduceOptions(value);
  const toggleCalc = (calc: string) => {
    const has = ro.calcs.includes(calc);
    onChange({ ...ro, calcs: has ? ro.calcs.filter((c) => c !== calc) : [...ro.calcs, calc] });
  };

  return (
    <div className="grid gap-2" data-options-group="reduceOptions">
      <div className="font-medium text-muted">Value (reduce)</div>
      <div className="grid gap-1 text-muted">
        Calculation
        <div className="flex flex-wrap gap-1.5">
          {CALCS.map((calc) => (
            <Button
              key={calc}
              variant={ro.calcs.includes(calc) ? "default" : "outline"}
              size="sm"
              aria-label={`reduce calc ${calc}`}
              aria-pressed={ro.calcs.includes(calc)}
              className="h-auto px-2 py-0.5 text-[11px]"
              onClick={() => toggleCalc(calc)}
            >
              {calc}
            </Button>
          ))}
        </div>
      </div>
      <ToggleField label="Show all values" checked={!!ro.values} onChange={(v) => onChange({ ...ro, values: v })} />
    </div>
  );
}
