// The source-picker model — now provided by the reusable `@nube/source-picker` package
// (source-picker-package-scope.md). This file is the SHELL SHIM: it re-exports the package model so the
// dashboard's many consumers (WidgetBuilder, QueryTab, VariableEditor, JsonPayloadField, the tests)
// keep their existing `./sourcePicker` import path, while the actual logic + types live in the package
// (one implementation, reusable from an extension too). The only shell-specific piece is the
// POSITIONAL `buildSourceEntries(series, rows, flows, descriptors)` signature the dashboard call sites
// use — kept here as a thin adapter over the package's object-form `buildSourceEntries({...})`.

import {
  buildSourceEntries as buildEntries,
  type ExtRow as PkgExtRow,
  type Flow as PkgFlow,
  type NodeDescriptor as PkgNodeDescriptor,
} from "@nube/source-picker";

export type { SourceEntry } from "@nube/source-picker";
export {
  seriesEntries,
  liveEntries,
  extensionEntries,
  extWidgetEntries,
  flowsEntries,
  sqlSourceEntry,
  SQL_SOURCE_ID,
  widgetIdOf,
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
