// The source-picker model — now provided by the reusable `@nube/source-picker` package
// (source-picker-package-scope.md). This file is the SHELL SHIM: it re-exports the package model so the
// dashboard's many consumers (QueryTab, VariableEditor, JsonPayloadField, the tests)
// keep their existing `./sourcePicker` import path, while the actual logic + types live in the package
// (one implementation, reusable from an extension too). The only shell-specific piece is the
// POSITIONAL `buildSourceEntries(series, rows, flows, descriptors)` signature the dashboard call sites
// use — kept here as a thin adapter over the package's object-form `buildSourceEntries({...})`.

import {
  buildSourceEntries as buildEntries,
  type ExtRow as PkgExtRow,
  type Flow as PkgFlow,
  type NodeDescriptor as PkgNodeDescriptor,
  type SourceEntry,
  SQL_SOURCE_ID,
} from "@nube/source-picker";

import type { Cell } from "@/lib/dashboard";

export type { SourceEntry, SourceGroup } from "@nube/source-picker";
export {
  seriesEntries,
  liveEntries,
  extensionEntries,
  extWidgetEntries,
  flowsEntries,
  sqlSourceEntry,
  SQL_SOURCE_ID,
  widgetIdOf,
  // The shared grouping primitive + canonical group-label lists — a consumer that renders its OWN
  // <select> (QueryTab’s shadcn Select) uses these instead of re-rolling
  // an `<optgroup>` renderer + a hardcoded label list (source-picker-package: one picker vocabulary).
  PickerGroup,
  READ_SOURCE_GROUPS,
  BUILDER_SOURCE_GROUPS,
  // The searchable combobox variant — QueryTab uses this instead of the plain grouped <select> so the
  // author types to filter across every source group (data-studio-ux: the first 10 seconds).
  SourceCombobox,
} from "@nube/source-picker";

/** The dashboard's positional signature (unchanged for every call site) → the package's object form.
 *  The shell's `@/lib` row types are structural supersets of the package's, so they pass through. */
export function buildSourceEntries(
  seriesNames: string[],
  rows: PkgExtRow[],
  flows: PkgFlow[] = [],
  descriptors: PkgNodeDescriptor[] = [],
) {
  return buildEntries({ series: seriesNames, extensions: rows, flows, descriptors });
}

/** Find the picker entry id that produced a cell's source (edit-mode seeding — lived in the deleted
 *  legacy `WidgetBuilder`; QueryTab still seeds its picker select with it). Matches a packaged tile by
 *  its `ext:` view, the SQL source by `store.query`, else the read/action tool (+ series arg). */
export function seedEntryId(cell: Cell | undefined, entries: SourceEntry[]): string {
  if (!cell) return "";
  const view = cell.view ?? "";
  if (view.startsWith("ext:")) return entries.find((e) => e.viewKey === view)?.id ?? "";
  if (cell.source?.tool === "store.query") return SQL_SOURCE_ID;
  const tool = cell.source?.tool || cell.action?.tool;
  if (!tool) return "";
  const series = (cell.source?.args as { series?: string } | undefined)?.series;
  // A rule target keys on its `rule_id` — every rule shares `tool === "rules.run"`, so tool alone would
  // collapse them all to the first rule entry (exactly as `series` disambiguates the shared `series.read`).
  const ruleId = (cell.source?.args as { rule_id?: string } | undefined)?.rule_id;
  const match = entries.find(
    (e) =>
      (e.source?.tool === tool &&
        (series === undefined ||
          (e.source?.args as { series?: string } | undefined)?.series === series) &&
        (ruleId === undefined ||
          (e.source?.args as { rule_id?: string } | undefined)?.rule_id === ruleId)) ||
      e.action?.tool === tool,
  );
  return match?.id ?? "";
}
