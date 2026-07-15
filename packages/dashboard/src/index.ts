// `@nube/dashboard` — public surface.
//
// The Lazybones dashboard GRID CORE, extracted from the shell so every Nube surface hosts ONE
// grid implementation. Cells + data in, DOM out: the package never fetches, persists, or knows
// a workspace exists. Persistence is the consumer's `onLayout`/… callbacks; data is the
// consumer's registered renderers (the injected-seam doctrine, one level up from
// `@nube/insights`'s client). Consumed two ways:
//   - `workspace:*` from the lb `ui/` app (a later migration slice),
//   - pinned externally via a git subdir dep + `import '@nube/dashboard/style.css'`.
//
// Three layers, adopt what you need: the MODEL (pure), the REGISTRY, the GRID/STACK hosts.

import "./dashboard.css";

// Model (pure) — the cell vocabulary + geometry helpers. No React, no CSS.
export {
  GRID_COLS,
  GRID_ROW_PX,
  canonicalView,
  cellSources,
  cellPrimaryTarget,
  cellView,
  cellFieldConfig,
  cellLabel,
  bindingSeries,
  bindingTags,
} from "./dashboard.types";
export type {
  WidgetType,
  View,
  DataSourceRef,
  Target,
  QueryOptions,
  Transformation,
  Binding,
  Source,
  Action,
  Visibility,
  Toolbar,
  Cell,
  Dashboard,
  DashboardSummary,
} from "./dashboard.types";

// Model (pure) — the Grafana fieldConfig TYPES (data only; the apply/format bridge is NOT here).
export { emptyFieldConfig } from "./fieldconfig.types";
export type {
  FieldConfig,
  FieldOptions,
  FieldOverride,
  Matcher,
  ValueMapping,
  ValueMappingResult,
  ThresholdsConfig,
  FieldColor,
  FieldColorModeId,
  DataLink,
} from "./fieldconfig.types";

// Model (pure) — panel-rows section math: positional membership + collapse visibility.
export {
  ROW_W,
  ROW_H,
  isRow,
  isCollapsed,
  rowOptions,
  rows,
  rowMembers,
  ungroupedCells,
  visibleCells,
} from "./rows";
export type { RowOptions } from "./rows";

// Model (pure) — the drag/resize merge (incl. the row-carries-its-members math) + helpers.
export { mergeLayout } from "./layout";
export type { LayoutItem } from "./layout";
export { timeOverrideBadge } from "./timeOverrideBadge";
export type { TimeRange } from "./timerange";

// The widget registry — the consumer's view → renderer map (the WidgetHost seam).
export { createRegistry, UnknownView, EXT_WILDCARD } from "./registry";
export type { WidgetRegistry, WidgetRenderer, WidgetRenderProps } from "./registry";

// The hosts — the editable/read-only grid (degrades to the stack below `stackBelow`) and the
// standalone read-only mobile stack.
export { DashboardGrid, FALLBACK_WIDTH } from "./Grid";
export type { DashboardGridProps } from "./Grid";
export { DashboardStack } from "./Stack";
export type { DashboardStackProps } from "./Stack";
export { RowHeader } from "./RowHeader";
export type { RowHeaderProps } from "./RowHeader";
