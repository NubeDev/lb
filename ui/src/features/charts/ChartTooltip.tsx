// The themed chart tooltip — one card, the x label on top, then a swatch + series name + right-aligned
// formatted value per series. Replaces Recharts' default tooltip (which ignores the app tokens) so the
// hover surface matches popovers everywhere. Numbers format through the caller's `valueFormatter` (the
// dashboard passes its user-prefs bridge; the channel passes a plain localiser) — never a local toFixed.
//
// One responsibility: render tooltip content from Recharts' payload.

import { tooltipSurface } from "./chartTheme";
import { formatXTick } from "./axisFormat";

/** The subset of Recharts' tooltip payload we read (v3 types it loosely; we narrow to what we draw). */
interface TooltipEntry {
  dataKey?: string | number;
  name?: string | number;
  value?: number | string;
}

interface Props {
  active?: boolean;
  payload?: TooltipEntry[];
  label?: unknown;
  valueFormatter: (n: number) => string;
  /** Colors keyed by series dataKey, so the swatch matches the drawn line/bar. */
  colorOf: (key: string) => string;
}

export function ChartTooltip({ active, payload, label, valueFormatter, colorOf }: Props) {
  if (!active || !payload || payload.length === 0) return null;
  return (
    <div style={tooltipSurface} role="tooltip">
      <div className="mb-1 font-medium text-fg/90">{formatXTick(label)}</div>
      <div className="flex flex-col gap-1">
        {payload.map((entry) => {
          const key = String(entry.dataKey ?? entry.name ?? "");
          const value = typeof entry.value === "number" ? entry.value : Number(entry.value);
          return (
            <div key={key} className="flex items-center gap-2">
              <span
                aria-hidden
                className="inline-block size-2.5 shrink-0 rounded-[3px]"
                style={{ background: colorOf(key) }}
              />
              <span className="text-muted">{entry.name}</span>
              <span className="ml-auto pl-4 font-medium tabular-nums text-fg">
                {Number.isFinite(value) ? valueFormatter(value) : "—"}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
