// The interactive plot builder — the "run the query, see the fields, decide how to plot" UX. Given the
// typed fields of a result and its rows, it lets the user pick a chart type, assign the X axis, toggle
// one or more numeric Y series, optionally split by a category (series), and tune per-type modifiers —
// with a LIVE preview (the same `PlotChart` the surface renders) updating on every change. Both surfaces
// mount this: the dashboard panel editor and the in-channel query result. It emits a `PlotSpec`; it does
// not persist (the surface owns save) — one model, one renderer, one builder.
//
// One responsibility: edit a PlotSpec against a set of fields, with a live preview.

import { useMemo } from "react";
import { AreaChart, BarChart3, BarChartBig, LineChart, PieChart, ScatterChart } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { cn } from "@/lib/utils";
import { numericFields, type FieldInfo, type PlotSpec, type PlotType } from "@/lib/charts";

import { PlotChart } from "./PlotChart";

interface Props {
  fields: FieldInfo[];
  rows: Array<Record<string, unknown>>;
  spec: PlotSpec;
  onChange: (spec: PlotSpec) => void;
  valueFormatter?: (n: number) => string;
  className?: string;
}

const TYPES: { type: PlotType; label: string; icon: typeof LineChart }[] = [
  { type: "line", label: "Line", icon: LineChart },
  { type: "area", label: "Area", icon: AreaChart },
  { type: "bar", label: "Bar", icon: BarChart3 },
  { type: "scatter", label: "Scatter", icon: ScatterChart },
  { type: "pie", label: "Pie", icon: PieChart },
  { type: "histogram", label: "Histogram", icon: BarChartBig },
];

export function PlotBuilder({ fields, rows, spec, onChange, valueFormatter, className }: Props) {
  const nums = useMemo(() => numericFields(fields), [fields]);
  const categories = useMemo(() => fields.filter((f) => f.kind !== "number").map((f) => f.name), [fields]);
  const set = (patch: Partial<PlotSpec>) => onChange({ ...spec, ...patch });

  const toggleY = (name: string) => {
    const has = spec.yFields.includes(name);
    set({ yFields: has ? spec.yFields.filter((f) => f !== name) : [...spec.yFields, name] });
  };

  const needsX = spec.type !== "histogram";
  const singleY = spec.type === "pie" || spec.type === "histogram";
  const canStack = spec.type === "bar" || spec.type === "area";
  const canSmooth = spec.type === "line" || spec.type === "area";

  return (
    <div className={cn("flex flex-col gap-4 lg:flex-row", className)}>
      {/* controls */}
      <div className="flex min-w-0 flex-col gap-4 lg:w-64 lg:shrink-0">
        {/* chart type */}
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-xs font-medium text-muted">Chart type</legend>
          <div className="grid grid-cols-3 gap-1.5" role="radiogroup" aria-label="chart type">
            {TYPES.map(({ type, label, icon: Icon }) => {
              const active = spec.type === type;
              return (
                <Button
                  key={type}
                  type="button"
                  role="radio"
                  aria-checked={active}
                  aria-label={label}
                  variant={active ? "default" : "outline"}
                  onClick={() => set({ type })}
                  className={cn(
                    "h-auto flex-col gap-1 rounded-lg px-2 py-2 text-[10px] font-medium",
                    active ? "border-accent/60" : "text-muted hover:border-accent/30 hover:text-fg",
                  )}
                >
                  <Icon size={16} aria-hidden />
                  {label}
                </Button>
              );
            })}
          </div>
        </fieldset>

        {/* X axis */}
        {needsX && (
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-muted">{spec.type === "pie" ? "Category" : "X axis"}</span>
            <Select value={spec.xField} onChange={(e) => set({ xField: e.target.value })} aria-label="x axis field">
              <option value="">— pick a field —</option>
              {fields.map((f) => (
                <option key={f.name} value={f.name}>
                  {f.name} ({f.kind})
                </option>
              ))}
            </Select>
          </label>
        )}

        {/* Y series */}
        <fieldset className="flex flex-col gap-1.5">
          <legend className="text-xs font-medium text-muted">{singleY ? "Value" : "Y series"}</legend>
          {nums.length === 0 ? (
            <p className="text-xs text-muted">No numeric fields to plot.</p>
          ) : (
            <div className="flex flex-wrap gap-1.5">
              {nums.map((name) => {
                const active = singleY ? spec.yFields[0] === name : spec.yFields.includes(name);
                return (
                  <Button
                    key={name}
                    type="button"
                    size="sm"
                    aria-pressed={active}
                    variant={active ? "default" : "outline"}
                    onClick={() => (singleY ? set({ yFields: [name] }) : toggleY(name))}
                    className={cn(
                      "h-7 px-2 tabular-nums",
                      active ? "border-accent/60" : "text-muted hover:border-accent/30 hover:text-fg",
                    )}
                  >
                    {name}
                  </Button>
                );
              })}
            </div>
          )}
        </fieldset>

        {/* Series split (long→wide) */}
        {!singleY && categories.length > 0 && (
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-muted">
              Split by <span className="font-normal text-muted/70">(optional)</span>
            </span>
            <Select
              value={spec.seriesField ?? ""}
              onChange={(e) => set({ seriesField: e.target.value || undefined })}
              aria-label="split by field"
            >
              <option value="">— none —</option>
              {categories.map((name) => (
                <option key={name} value={name}>
                  {name}
                </option>
              ))}
            </Select>
          </label>
        )}

        {/* modifiers */}
        <div className="flex flex-col gap-2">
          {canSmooth && <ToggleRow label="Smooth curve" checked={!!spec.smooth} onChange={(v) => set({ smooth: v })} />}
          {canStack && <ToggleRow label="Stack series" checked={!!spec.stacked} onChange={(v) => set({ stacked: v })} />}
          {spec.type === "bar" && (
            <ToggleRow label="Horizontal bars" checked={!!spec.horizontal} onChange={(v) => set({ horizontal: v })} />
          )}
          {spec.type === "histogram" && (
            <label className="flex items-center justify-between gap-2">
              <span className="text-xs text-fg/80">Buckets</span>
              <Input
                type="number"
                min={2}
                max={50}
                value={spec.bins ?? 12}
                onChange={(e) => set({ bins: Math.max(2, Math.min(50, Number(e.target.value) || 12)) })}
                className="h-8 w-20 tabular-nums"
                aria-label="histogram buckets"
              />
            </label>
          )}
        </div>
      </div>

      {/* live preview */}
      <div className="flex min-w-0 flex-1 flex-col gap-1.5">
        <div className="flex items-center gap-2">
          <span className="text-xs font-medium text-muted">Preview</span>
          <Badge variant="outline" className="tabular-nums">
            {rows.length} rows
          </Badge>
        </div>
        <div className="flex min-h-[220px] flex-1 rounded-lg border border-border bg-panel/40 p-2">
          <PlotChart rows={rows} spec={spec} valueFormatter={valueFormatter} ariaLabel="plot preview" />
        </div>
      </div>
    </div>
  );
}

function ToggleRow({ label, checked, onChange }: { label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return (
    <label className="flex items-center justify-between gap-2">
      <span className="text-xs text-fg/80">{label}</span>
      <Switch checked={checked} onCheckedChange={onChange} aria-label={label} />
    </label>
  );
}
