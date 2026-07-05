// The rules data explorer — a THIN host adapter over `@nube/source-picker`'s `<CatalogExplorer>`
// (system-catalog scope). The package owns the explorer skin (sections, per-state rendering, the
// table→column tree); this file is the ONE place the shell's `@/lib/*` API clients meet the
// package's injected `SourceLoaders`, plus the rule's host-owned snippet mapping (the package never
// knows what a pick MEANS — rule 10).
//
// What a click yields per section (the host's `onSelect` mapping):
//   - datasource → `source("name")` (a rule queries it by registered name);
//   - table      → `name` (a bare table identifier);
//   - column     → `column` (a bare column identifier);
//   - series     → `history("series", "name", "24h")` (read 24h of history).
//
// `useDataExplorer` is retired — the package's `useCatalog` is the one loader orchestration now.

import { useMemo } from "react";

import { CatalogExplorer, useCatalog, type CatalogEntry, type SourceLoaders } from "@nube/source-picker";

import { listDatasources } from "@/lib/datasources";
import { readSchema } from "@/lib/schema";
import { listRealSeries } from "@/lib/ingest/schema.api";

interface DataExplorerProps {
  ws: string;
  /** Insert a snippet at the editor cursor (the parent owns the editor transaction). */
  onInsert: (snippet: string) => void;
}

/** Build the shell adapter once per render — the package reads it via a ref so an unmemoized object
 *  does NOT loop the hook (the package's own discipline; same as `useSourcePicker`). */
function shellLoaders(): SourceLoaders {
  return {
    listDatasources: () => listDatasources(),
    readSchema: () => readSchema(),
    listSeries: () => listRealSeries(),
  };
}

/** The data explorer — the package's `<CatalogExplorer>` plus the rule's snippet mapping. */
export function DataExplorer({ ws, onInsert }: DataExplorerProps) {
  // `loaders` is read via ref inside `useCatalog`, so a fresh literal per render does not loop the
  // effect (keyed on `ws` only). Keep it simple — no `useMemo` needed for correctness.
  const loaders = useMemo(shellLoaders, []);
  const sections = useCatalog(loaders, ws);
  return <CatalogExplorer sections={sections} onSelect={(e) => onInsert(snippetFor(e))} />;
}

/** Map a picked catalog entry onto the rule's Rhai snippet. The host owns this mapping; the package
 *  returns a tagged entry and never branches on host meaning. */
function snippetFor(entry: CatalogEntry): string {
  switch (entry.kind) {
    case "datasource":
      return `source(${JSON.stringify(entry.name)})`;
    case "table":
      return entry.table;
    case "column":
      return entry.column;
    case "series":
      return `history("series", ${JSON.stringify(entry.name)}, "24h")`;
    case "channel":
    case "insight":
    case "inbox":
      // Not shown in the rules panel today (no matching loader wired). If a future rule surface
      // wires one of these, add its snippet here — the entry carries the fields needed.
      return entry.id;
  }
}
