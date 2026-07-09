// The built-in `weather` view (weather scope): current conditions from `weather.current`, rendered
// as a shadcn Card. Data comes ONLY through `usePanelData` (the one panel-data hook, invariant A) —
// no direct bridge call here. One responsibility: render a weather cell from a cell.

import { Card, CardContent, CardHeader } from "@/components/ui/card";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetMessage } from "../../widgets/chrome";
import { usePanelData } from "../../builder/usePanelData";
import { wmoCondition } from "./wmoCode";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function WeatherPanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const row = rows[0];
  const tempC = typeof row?.temp_c === "number" ? row.temp_c : null;
  const windKph = typeof row?.wind_kph === "number" ? row.wind_kph : null;
  const code = typeof row?.code === "number" ? row.code : null;
  const observedTs = typeof row?.observed_ts === "string" ? row.observed_ts : null;
  const location = typeof row?.location === "string" ? row.location : null;

  if (tempC === null) return <WidgetMessage tone="muted">no value yet</WidgetMessage>;

  return (
    <Card className="h-full justify-between border-none bg-panel shadow-none" aria-label="weather panel">
      <CardHeader className="pb-0">
        <div className="truncate text-xs font-medium" style={{ color: "hsl(var(--muted))" }}>
          {label ?? location ?? "Weather"}
        </div>
      </CardHeader>
      <CardContent className="flex flex-1 flex-col justify-center gap-1 pt-2">
        <span
          className="text-3xl font-semibold tabular-nums"
          style={{ color: "hsl(var(--fg))" }}
          aria-label="weather temp"
        >
          {tempC.toFixed(1)}°C
        </span>
        <span className="text-xs" style={{ color: "hsl(var(--fg))" }} aria-label="weather condition">
          {code !== null ? wmoCondition(code) : "—"}
        </span>
        <span className="text-xs" style={{ color: "hsl(var(--muted))" }} aria-label="weather wind">
          Wind {windKph !== null ? windKph.toFixed(1) : "—"} km/h
        </span>
        <span className="text-[0.65rem]" style={{ color: "hsl(var(--muted))" }} aria-label="weather updated">
          {observedTs ? `Updated ${observedTs.replace("T", " ")}` : ""}
        </span>
      </CardContent>
    </Card>
  );
}
