// The Widget Kit library public API (widget-kit scope, Phase 1) — the ONE import surface every consuming
// surface (palette arg rail, dashboard, channel responses, a future ext host) uses: the widget REGISTRY
// (the public resolver), the per-field PRESENTATION resolver + humanize fallback, the shared TABLE
// column-model, and the widget/field TYPES. Re-export only (FILE-LAYOUT). The built-in input widgets are
// under `./inputs` (a widget is resolved by string via the registry, then imported from there).
export * from "./types";
export { resolveWidget, TEXT_ENTRY } from "./registry";
export type { WidgetId, WidgetEntry } from "./registry";
export { resolveFieldPresentation } from "./presentation/resolve";
export { humanize } from "./presentation/humanize";
export { columnsOf, resolveColumns, cellText } from "./table/columns";
export type { TableColumn } from "./table/columns";
