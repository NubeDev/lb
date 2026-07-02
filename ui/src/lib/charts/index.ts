// The shared chart model — the ONE plot-spec/field-typing/transform library both chart surfaces reuse
// (dashboard panels + in-channel query results). Import from `@/lib/charts`, never the deep files.

export type { PlotSpec, PlotType } from "./plotSpec";
export {
  emptyPlotSpec,
  isPlottable,
  readPlotSpec,
  CATEGORICAL_TYPES,
  CONTINUOUS_TYPES,
} from "./plotSpec";
export type { FieldKind, FieldInfo } from "./fieldKind";
export { inferFields, numericFields, defaultXField } from "./fieldKind";
export type { PlotFrame, PlotSeries } from "./buildPlot";
export { buildPlot } from "./buildPlot";
export { suggestPlot, suggestFromFields } from "./suggestPlot";
