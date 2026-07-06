// The pages-as-panes registry (data-studio-10x scope, phase 2) — the view kinds the workbench's
// "+ Open view" menu can mount as dock panes, each mapping to the REAL routed view component
// (`FlowsView`, `RulesView`, …): same code path, same gateway, same caps — never a re-implementation.
// Kinds are plain data; gating rides each surface's existing `allowed` route lens (the gateway
// re-checks every verb server-side regardless). Data Studio itself is deliberately absent — the
// host surface never lists itself (no recursive embedding). Extension pages are a later cut via the
// generic `ext.list` discovery (rule 10) — no extension id appears here. First cut: ONE pane per
// view kind (pages weren't written to be multi-mounted; the menu re-activates an open pane).
//
// The icon + title per kind are NOT redefined here — they come from the shared `SURFACE_DEF` map
// (`ui/src/features/shell/surfaceDefs.ts`) so the dock tab, the "+ Open view" menu, and the sidebar
// rail all show the SAME icon for a given surface. Update it once there; every consumer inherits.

import type { LucideIcon } from "lucide-react";
import { Database } from "lucide-react";

import { DataView } from "@/features/data";
import { DatasourcesAdmin, DatasourceDetailPage } from "@/features/datasources";
import { FlowsView } from "@/features/flows";
import { IngestView } from "@/features/ingest";
import { QueryWorkbench } from "@/features/query-workbench";
import { RulesView } from "@/features/rules";
import { SURFACE_DEF } from "@/features/shell/surfaceDefs";
import type { CoreSurface } from "@/features/shell";
import { Button } from "@/components/ui/button";

export interface ViewPaneProps {
  ws: string;
  /** The pane's persisted in-pane selection (open flow/rule/datasource) — the pane's own "URL". */
  sel: string | null;
  onSel: (sel: string | null) => void;
}

export interface ViewPaneDef {
  /** The pane kind — persisted in the layout record's params; treated as opaque data by the dock.
   *  Always identical to the surface key, so the shared `SURFACE_DEF` lookup works. */
  kind: string;
  title: string;
  icon: LucideIcon;
  /** The `allowed` surface that gates this entry in the "+ Open view" menu (UI lens only). */
  surface: CoreSurface;
  Component: (props: ViewPaneProps) => JSX.Element;
}

/** Datasources drills to a detail in-pane (the routed page navigates; a pane has no URL, so the
 *  selection lives in the pane's params with a compact back strip — both pages are the REAL ones). */
function DatasourcesPane({ ws, sel, onSel }: ViewPaneProps) {
  if (!sel) return <DatasourcesAdmin ws={ws} onOpen={onSel} />;
  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex items-center border-b border-border px-2 py-1">
        <Button aria-label="back to datasources" size="sm" variant="ghost" onClick={() => onSel(null)}>
          ← All datasources
        </Button>
      </div>
      <div className="min-h-0 flex-1">
        <DatasourceDetailPage ws={ws} name={sel} />
      </div>
    </div>
  );
}

/** The Query workbench as a Data Studio pane (query-workbench-view scope, slice 3). The same
 *  `QueryWorkbench` the Datasources page + the Data page mount, pinned to the platform's native
 *  store (`store.schema`/`store.query`, surreal dialect). A federation query in Data Studio goes
 *  through the existing `datasources` pane (which drills into `DatasourceDetail` → `QueryWorkbench`
 *  over the federation source). `kind: "query"` is a distinct pane key (no new `CoreSurface` — the
 *  slice's retargeted registration item 4: "one VIEW_PANES line keyed to an existing surface");
 *  gating rides the `data` surface lens (member-level `mcp:store.query:call` is re-checked per run). */
function QueryPane({ ws, sel, onSel }: ViewPaneProps) {
  return <QueryWorkbench ws={ws} source="surreal-local" sel={sel} onSel={onSel} />;
}

/** Build a registry entry: the icon + title default from the shared `SURFACE_DEF` so a dock tab, the
 *  "+ Open view" menu, and the sidebar rail never drift apart for a given surface. */
function def(
  surface: CoreSurface,
  Component: (props: ViewPaneProps) => JSX.Element,
): ViewPaneDef {
  const s = SURFACE_DEF[surface];
  return { kind: surface, title: s.label, icon: s.icon, surface, Component };
}

/** Build a registry entry whose `kind` is DISTINCT from its gating `surface` (used when a pane needs
 *  its own persisted layout key + dock identity but no new `CoreSurface`). The title/icon default
 *  from the gating surface's def, overridable. */
function defAs(
  kind: string,
  surface: CoreSurface,
  Component: (props: ViewPaneProps) => JSX.Element,
  title?: string,
  icon?: LucideIcon,
): ViewPaneDef {
  const s = SURFACE_DEF[surface];
  return { kind, title: title ?? s.label, icon: icon ?? s.icon, surface, Component };
}

export const VIEW_PANES: ViewPaneDef[] = [
  def("flows", ({ ws, sel, onSel }) => <FlowsView ws={ws} flowId={sel} onSelectFlow={onSel} />),
  def("rules", ({ ws, sel, onSel }) => <RulesView ws={ws} ruleId={sel} onSelectRule={onSel} />),
  def("data", ({ ws }) => <DataView ws={ws} />),
  def("datasources", DatasourcesPane),
  defAs("query", "data", QueryPane, "Query", Database),
  def("ingest", ({ ws }) => <IngestView ws={ws} />),
];

export function viewPane(kind: string): ViewPaneDef | undefined {
  return VIEW_PANES.find((p) => p.kind === kind);
}
