// The chart point-budget context (data-studio builder-ergonomics session). Hosts that render MANY
// charts of the same frames (the builder's viz gallery: 6 live thumbnails) shrink the budget for
// their subtree; everything else gets the full-size default. Read at the chart chokepoints
// (`recharts.tsx` series charts, `PlotChart`) next to `downsamplePoints`/`downsampleRows`.
//
// One responsibility: carry "how many points is this chart worth drawing".

import { createContext, useContext } from "react";

/** A full-width panel is ~1–2k px; more points than that is invisible. */
export const DEFAULT_CHART_BUDGET = 1500;

const ChartBudgetContext = createContext(DEFAULT_CHART_BUDGET);

export function ChartBudgetProvider({ budget, children }: { budget: number; children: React.ReactNode }) {
  return <ChartBudgetContext.Provider value={budget}>{children}</ChartBudgetContext.Provider>;
}

export function useChartBudget(): number {
  return useContext(ChartBudgetContext);
}
