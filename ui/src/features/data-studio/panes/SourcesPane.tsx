// The Sources rail tab (data-studio-10x scope, goal 5) — a `CatalogExplorer` HOST: the workspace
// system catalog (system-catalog scope) as ONE browsable tree — datasources, local tables → columns,
// series, channels, insights — with the package's honest per-section deny/loading/empty states.
// Replaces the bare `SourcePicker` select. This file is the ONE place the shell's `@/lib/*` clients
// meet the package's injected `SourceLoaders`, plus the studio's host-owned pick mapping (click →
// OPEN A BUILDER TAB on the entry; the package never knows what a pick MEANS — rule 10, exactly like
// the rules panel's Rhai mapping). One responsibility: the loaders + the entry→builder-source map.

import { useMemo } from "react";

import {
  CatalogExplorer,
  useCatalog,
  type CatalogEntry,
  type SourceLoaders,
  type SourceSelection,
} from "@nube/source-picker";

import { listDatasources } from "@/lib/datasources";
import { readSchema } from "@/lib/schema";
import { listRealSeries } from "@/lib/ingest/schema.api";
import { listChannels } from "@/lib/channel/channel.api";
import { listInsights } from "@/lib/insights/insights.api";

interface Props {
  ws: string;
  /** Open a builder tab on the picked entry (`label` becomes the tab name). */
  onOpen: (sel: SourceSelection, label: string) => void;
}

/** The shell adapter — read via ref inside `useCatalog`, keyed on `ws` (the package's discipline). */
function shellLoaders(ws: string): SourceLoaders {
  return {
    listDatasources: () => listDatasources(),
    readSchema: () => readSchema(),
    listSeries: () => listRealSeries(),
    listChannels: () => listChannels(ws),
    listInsights: () => listInsights({}).then((page): { id: string; title: string }[] => page.items),
  };
}

export function SourcesPane({ ws, onOpen }: Props) {
  const loaders = useMemo(() => shellLoaders(ws), [ws]);
  const sections = useCatalog(loaders, ws);
  return (
    <div className="flex flex-col gap-2">
      <p className="px-1 text-xs text-muted">Pick a catalog entry to open it in a builder tab.</p>
      <CatalogExplorer
        sections={sections}
        onSelect={(entry) => {
          const mapped = selectionFor(entry);
          if (mapped) onOpen(mapped.sel, mapped.label);
        }}
      />
    </div>
  );
}

/** Map a picked catalog entry onto a builder READ SOURCE (`{tool,args}` — the same shape every
 *  `viz.query` target takes; the gateway re-checks each tool's own cap per call). The host owns
 *  this mapping — the package returns a tagged entry and never branches on host meaning. */
function selectionFor(entry: CatalogEntry): { sel: SourceSelection; label: string } | null {
  const pick = (tool: string, args: Record<string, unknown>, label: string) => ({
    sel: { id: entry.id, source: { tool, args } },
    label,
  });
  switch (entry.kind) {
    case "datasource":
      // The SQL is the builder's to write — the query editor opens prefilled on the source.
      return pick("federation.query", { source: entry.name, sql: "" }, entry.name);
    case "table":
      return pick("store.query", { sql: `SELECT * FROM ${entry.table} LIMIT 100` }, entry.table);
    case "column":
      return pick(
        "store.query",
        { sql: `SELECT ${entry.column} FROM ${entry.table} LIMIT 100` },
        `${entry.table}.${entry.column}`,
      );
    case "series":
      return pick("series.read", { series: entry.name }, entry.name);
    case "channel":
      return pick("inbox.list", { channel: entry.name }, entry.name);
    case "insight":
      return pick("insight.get", { id: entry.id.replace(/^insight:/, "") }, entry.title);
    default:
      // A catalog kind with no builder meaning yet (e.g. inbox items) — no tab, no crash.
      return null;
  }
}
