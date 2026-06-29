// The v3 `stat` panel renderer (viz chart-types scope, Phase 2). The big single value of a frame:
// `reduceOptions` collapses the rows to one value (the VISIBLE frame→value bridge), `fieldConfig`
// formats it through the ONE user-prefs bridge (`format.ts` — never a local `toFixed`/unit string), and
// `colorMode` paints it (value text / panel background) by the threshold color. Data comes ONLY through
// `usePanelData` (invariant A); there is no client transform (invariant B — reduceOptions is a per-viz
// reduce, not the pipeline). It replaces the v2 StatView's single-`unit`-string render.
//
// One responsibility: render a stat panel from a cell. The reduce, options, field math, and formatting
// each live in their own file; this composes them.

import type { ReactNode } from "react";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { SeriesLineChart } from "../../widgets/recharts";
import { usePanelData } from "../../builder/usePanelData";
import { rowNumber } from "../num";
import { reduceFrame } from "../reduce";
import { valueFieldOptions, valueColor } from "../field";
import { readStatOptions } from "./options";
import { formatValue } from "../../fieldconfig/format";
import { applyMappings } from "../../fieldconfig/mappings";
import { resolveColor } from "../../fieldconfig/color";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

export function StatPanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;

  const options = readStatOptions(cell.options);
  const opts = valueFieldOptions(cell);

  // The single value the stat draws — the explicit reduceOptions bridge (no implicit guess).
  const value = reduceFrame(rows, options.reduceOptions);
  if (value === null) {
    // Honest empty: a mapped/noValue label when the field declares one, else "no value" — never a fake 0.
    const mapped = applyMappings(null, opts.mappings);
    const text = mapped?.text ?? opts.noValue;
    if (!text) return <WidgetMessage tone="muted">no value yet</WidgetMessage>;
    return <StatBody label={label ?? ""} text={text} color="hsl(var(--fg))" colorMode="none" sparkline={null} />;
  }

  // A value mapping wins over the numeric format (Grafana order); else the one user-prefs bridge.
  const mapped = applyMappings(value, opts.mappings);
  const color = mapped?.color ? resolveColor(mapped.color) : valueColor(value, opts);
  const text = mapped?.text ?? formatValue(value, opts).text;

  // The sparkline (graphMode: line|area) over the frame's points — colored to match the value.
  const points = rows.map(rowNumber).filter((n): n is number => n !== null);
  const sparkline =
    options.graphMode !== "none" && points.length > 1 ? (
      <div className="pointer-events-none h-8 w-full" style={{ color }}>
        <SeriesLineChart points={points} ariaLabel="stat sparkline" />
      </div>
    ) : null;

  return (
    <StatBody
      label={options.textMode === "none" || options.textMode === "value" ? "" : label ?? opts.displayName ?? ""}
      text={text}
      color={color}
      colorMode={options.colorMode}
      justify={options.justifyMode}
      sparkline={sparkline}
    />
  );
}

/** The big-value body — shared layout for the value + an optional sparkline, with `colorMode` painting
 *  either the value text (`value`) or the whole panel (`background`/`background_solid`). */
function StatBody({
  label,
  text,
  color,
  colorMode,
  justify = "auto",
  sparkline,
}: {
  label: string;
  text: string;
  color: string;
  colorMode: "value" | "background" | "background_solid" | "none";
  justify?: "auto" | "center";
  sparkline: ReactNode;
}) {
  const isBackground = colorMode === "background" || colorMode === "background_solid";
  const valueColorStyle = colorMode === "value" ? color : isBackground ? "hsl(var(--fg))" : undefined;
  const align = justify === "center" ? "items-center text-center" : "items-start";

  return (
    <div
      className="flex h-full flex-col rounded-md"
      style={isBackground ? { background: color, color: "hsl(var(--fg))" } : undefined}
      aria-label="stat panel"
      data-color-mode={colorMode}
    >
      <WidgetHeader label={label} />
      <div className={`flex flex-1 flex-col justify-center gap-1 ${align}`}>
        <span className="text-3xl font-semibold tabular-nums" style={valueColorStyle ? { color: valueColorStyle } : undefined} aria-label="stat value">
          {text}
        </span>
        {sparkline}
      </div>
    </div>
  );
}
