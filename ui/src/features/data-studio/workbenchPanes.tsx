// The pages-as-panes registry (data-studio-10x scope, phase 2) — the view kinds the workbench's
// "+ Open view" menu can mount as dock panes, each mapping to the REAL routed view component
// (`FlowsView`, `RulesView`, …): same code path, same gateway, same caps — never a re-implementation.
// Kinds are plain data; gating rides each surface's existing `allowed` route lens (the gateway
// re-checks every verb server-side regardless). Data Studio itself is deliberately absent — the
// host surface never lists itself (no recursive embedding). Extension pages are a later cut via the
// generic `ext.list` discovery (rule 10) — no extension id appears here. First cut: ONE pane per
// view kind (pages weren't written to be multi-mounted; the menu re-activates an open pane).

import { Cable, Database, GitBranch, Import, SlidersHorizontal } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { DataView } from "@/features/data";
import { DatasourcesAdmin, DatasourceDetailPage } from "@/features/datasources";
import { FlowsView } from "@/features/flows";
import { IngestView } from "@/features/ingest";
import { RulesView } from "@/features/rules";
import { Button } from "@/components/ui/button";
import type { CoreSurface } from "@/features/shell";

export interface ViewPaneProps {
  ws: string;
  /** The pane's persisted in-pane selection (open flow/rule/datasource) — the pane's own "URL". */
  sel: string | null;
  onSel: (sel: string | null) => void;
}

export interface ViewPaneDef {
  /** The pane kind — persisted in the layout record's params; treated as opaque data by the dock. */
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

export const VIEW_PANES: ViewPaneDef[] = [
  {
    kind: "flows",
    title: "Flows",
    icon: GitBranch,
    surface: "flows",
    Component: ({ ws, sel, onSel }) => <FlowsView ws={ws} flowId={sel} onSelectFlow={onSel} />,
  },
  {
    kind: "rules",
    title: "Rules",
    icon: SlidersHorizontal,
    surface: "rules",
    Component: ({ ws, sel, onSel }) => <RulesView ws={ws} ruleId={sel} onSelectRule={onSel} />,
  },
  {
    kind: "data",
    title: "Data",
    icon: Database,
    surface: "data",
    Component: ({ ws }) => <DataView ws={ws} />,
  },
  {
    kind: "datasources",
    title: "Datasources",
    icon: Cable,
    surface: "datasources",
    Component: DatasourcesPane,
  },
  {
    kind: "ingest",
    title: "Ingest",
    icon: Import,
    surface: "ingest",
    Component: ({ ws }) => <IngestView ws={ws} />,
  },
];

export function viewPane(kind: string): ViewPaneDef | undefined {
  return VIEW_PANES.find((p) => p.kind === kind);
}
