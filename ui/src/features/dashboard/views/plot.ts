// The dashboard bridge to the shared plot model — read a cell's persisted X/Y plot spec (`options.plot`)
// as a validated `PlotSpec`, and gate whether a panel should render through the shared `PlotChart` (real
// axes, multi-series) vs its legacy single-value renderer. A cell with no `plot` option keeps today's
// behavior byte-for-byte (backward compatible); one configured through the editor's Plot tab draws the
// full x/y chart. One responsibility: cell.options.plot → PlotSpec | null.

import type { Cell } from "@/lib/dashboard";
import { isPlottable, readPlotSpec, type PlotSpec } from "@/lib/charts";

/** The cell's configured plot spec, or null when none is set or it isn't drawable yet. */
export function cellPlot(cell: Cell): PlotSpec | null {
  const raw = (cell.options as Record<string, unknown> | undefined)?.plot;
  const spec = readPlotSpec(raw);
  return spec && isPlottable(spec) ? spec : null;
}
