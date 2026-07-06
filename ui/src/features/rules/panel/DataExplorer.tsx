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
//
// The `<CatalogExplorer>` UI is LAZY-LOADED via `React.lazy` so the picker bundle code-splits into
// its own chunk and is only paid for when a user actually opens the rules authoring panel.

import { lazy, Suspense, useMemo } from "react";

import { useCatalog, type CatalogEntry, type SourceLoaders } from "@nube/source-picker";

import { listDatasources } from "@/lib/datasources";
import { listQueries } from "@/lib/queries";
import { readSchema } from "@/lib/schema";
import { listRealSeries } from "@/lib/ingest/schema.api";

const CatalogExplorer = lazy(() =>
  import("@nube/source-picker").then((m) => ({ default: m.CatalogExplorer })),
);

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
    // Saved queries → the Saved-queries explorer section. A rule composes a saved query via
    // `source("query:<name>")` → `query.run {id}` (re-gated, no-widening) — the snippet mapping
    // turns a click into that source line. Reached through `mcp_call` (`query.list`).
    listQueries: () => listQueries(),
  };
}

/** The data explorer — the package's `<CatalogExplorer>` plus the rule's snippet mapping. */
export function DataExplorer({ ws, onInsert }: DataExplorerProps) {
  // `loaders` is read via ref inside `useCatalog`, so a fresh literal per render does not loop the
  // effect (keyed on `ws` only). Keep it simple — no `useMemo` needed for correctness.
  const loaders = useMemo(shellLoaders, []);
  const { sections, loadSection } = useCatalog(loaders, ws);
  return (
    <Suspense fallback={<CatalogExplorerFallback />}>
      <CatalogExplorer
        sections={sections}
        onLoadSection={loadSection}
        onSelect={(e) => onInsert(snippetFor(e))}
      />
    </Suspense>
  );
}

/** A tiny inline skeleton shown while the lazy picker chunk loads (rule 9: honest UI). */
function CatalogExplorerFallback() {
  return (
    <div aria-label="loading catalog" className="flex flex-col gap-1">
      <div className="h-4 w-full animate-pulse rounded-md bg-fg/10" />
      <div className="h-4 w-2/3 animate-pulse rounded-md bg-fg/10" />
    </div>
  );
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
    case "query":
      // A rule composes a saved query by name: `source("query:<id>")` resolves to `query.run {id}`
      // inside the cage (the rule still runs under `caller ∩ grant`; `query.run` re-checks the
      // target cap — no widening). The slug is the catalog id minus its `query:` prefix.
      return `source(${JSON.stringify(`query:${entry.id.replace(/^query:/, "")}`)})`;
    case "channel":
    case "insight":
    case "inbox":
      // Not shown in the rules panel today (no matching loader wired). If a future rule surface
      // wires one of these, add its snippet here — the entry carries the fields needed.
      return entry.id;
  }
}
