// The datasource detail page (datasources-ux scope) — the per-source explorer a non-SQL user lands
// on after clicking a datasource row. Two concerns, one screen:
//   1. Table discovery (the "Discovery" mode, left rail): click a table → see its columns, click
//      "Preview rows" → run a bounded `SELECT *` — no typing. The generated SQL is echoed read-only.
//   2. The Query mode (query-workbench-view scope, slice 3): the full `QueryWorkbench` — the
//      Builder⇄Code editor (canvas builder from slice 1 + schema-aware completion from slice 2),
//      Run, results grid, and the shipped Save/Open saved-query dialogs. The ad-hoc SQL `<Textarea>`
//      that used to live here is replaced by the workbench; the editor+run+results+saved-queries
//      state is now the workbench's, not the page's.
//
// All queries run through the real `federation.query` verb (workspace-pinned host-side, SELECT-only
// validated host + sidecar). Trusted shell code only — never an extension. One responsibility, one
// file (FILE-LAYOUT). shadcn-first per ui-standards-scope.

import { Suspense, lazy, useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft,
  Database,
  Eye,
  Loader2,
  Network,
  Table2,
  Wand2,
} from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { QueryWorkbench } from "@/features/query-workbench";
import { PICK_DASHBOARD } from "@/features/panel-builder/wizard/steps";
import { DatasourceProbe } from "./DatasourceProbe";
import { QueryResults } from "./QueryResults";
import { TableDiscovery } from "./TableDiscovery";
import { useDatasourceQuery } from "./useDatasourceQuery";
import { useDatasources } from "./useDatasources";
import type { DatasourceSummary, ProbeResult } from "@/lib/datasources";

// Code-split the schema ERD (and its `@xyflow/react` weight) so it only loads when Discovery → Diagram
// is toggled on (mirror of data/DataGraph's lazy chunk).
const SchemaErd = lazy(() => import("./erd/SchemaErd"));

// The Schemas tab is lazy too: mounting it is what fires `dbschema.list` (via SchemaDesignerList).
// Keeping it off the initial chunk means the MCP call only happens once the user clicks the tab.
const SchemaDesignerList = lazy(() =>
  import("./designer/SchemaDesignerList").then((m) => ({ default: m.SchemaDesignerList })),
);

interface Props {
  ws: string;
  source: DatasourceSummary;
  probe?: ProbeResult;
  onTest: (name: string) => void;
  onBack: () => void;
}

/** Discovery = the no-SQL browse (TableDiscovery + bounded preview OR the schema ERD diagram);
 *  Query = the full workbench (Builder⇄Code editor + Run + results + saved queries);
 *  Schemas = the designed-schema roster (schema-designer). Default Discovery (the legacy no-SQL
 *  affordance). Schemas is lazy: its list-verb fires only when the tab is selected. */
type Mode = "discovery" | "query" | "schemas";

/** The Discovery tab's two views: List (the table rail + bounded preview) vs Diagram (the schema ERD).
 *  Default List (the legacy no-SQL affordance); Diagram is a lazy chunk. */
type DiscoveryView = "list" | "diagram";

const PREVIEW_LIMIT = 100;

export function DatasourceDetail({ ws, source, probe, onTest, onBack }: Props) {
  const navigate = useNavigate();
  const q = useDatasourceQuery(source.name);
  const [mode, setMode] = useState<Mode>("discovery");
  const [discoveryView, setDiscoveryView] = useState<DiscoveryView>("list");
  const [selectedTable, setSelectedTable] = useState<string | null>(null);

  // Discover the table list on first mount (and when the source changes).
  useEffect(() => {
    void q.discoverTables();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source.name, source.kind]);

  const selectTable = (table: string) => {
    setSelectedTable(table);
    void q.describeTable(table);
  };

  const preview = (table: string) => {
    setSelectedTable(table);
    void q.describeTable(table);
    void q.previewTable(table, PREVIEW_LIMIT);
  };

  return (
    <section className="flex h-full min-w-0 flex-col bg-bg">
      <AppPageHeader
        icon={Database}
        title={source.name}
        description={`${source.kind} · ${source.endpoint}`}
        workspace={ws}
        actions={
          <>
            <Button
              aria-label="back to datasources"
              variant="ghost"
              size="sm"
              className="gap-1.5"
              onClick={onBack}
            >
              <ArrowLeft size={14} /> Back
            </Button>
            <DatasourceProbe name={source.name} result={probe} onTest={onTest} />
          </>
        }
      />

      <div className="flex items-center gap-2 border-b border-border bg-bg px-3 py-2">
        <ModeToggle mode={mode} onChange={setMode} />
        {mode === "discovery" && (
          <DiscoveryViewToggle view={discoveryView} onChange={setDiscoveryView} />
        )}
        <Badge variant="secondary" className="font-mono text-[11px]">
          secret:{source.secretRef}
        </Badge>
        {selectedTable && (
          <Badge variant="outline" className="font-mono text-[11px]">
            <Eye size={11} className="mr-1" />
            {selectedTable}
          </Badge>
        )}
        {q.loading && (
          <span className="inline-flex items-center gap-1.5 text-xs text-muted">
            <Loader2 size={12} className="animate-spin" /> running…
          </span>
        )}
        {q.error && (
          <span
            role="alert"
            className="ml-auto truncate text-xs text-destructive"
            title={q.error}
          >
            {q.error}
          </span>
        )}
      </div>

      <div className="flex min-h-0 flex-1 flex-col md:flex-row">
        {mode === "discovery" && (
          discoveryView === "diagram" ? (
            <div className="min-h-0 min-w-0 flex-1">
              <Suspense
                fallback={
                  <div className="flex h-full items-center justify-center bg-bg text-sm text-muted">
                    <Loader2 size={14} className="mr-2 animate-spin" /> Loading diagram…
                  </div>
                }
              >
                {/* The schema ERD — tables as nodes, dashed edges for naming-convention-inferred
                    relationships. Shares `selectedTable` with the rail: clicking a node selects it in
                    the same Discovery state (so flipping back to List keeps the selection). */}
                <SchemaErd
                  source={source.name}
                  tables={q.tables}
                  selectedTable={selectedTable}
                  onSelect={selectTable}
                />
              </Suspense>
            </div>
          ) : (
            <>
              <TableDiscovery
                tables={q.tables}
                selectedTable={selectedTable}
                columns={q.columns}
                loading={q.loading}
                onSelect={selectTable}
                onPreview={preview}
                onRefresh={() => void q.discoverTables()}
              />
              <div className="min-h-0 min-w-0 flex-1">
                <QueryResults
                  result={q.result}
                  emptyHint="Pick a table and preview its rows, or switch to Query mode to author SQL."
                />
              </div>
            </>
          )
        )}

        {mode === "query" && (
          <div className="min-h-0 min-w-0 flex-1">
            {/* The full workbench — Builder⇄Code editor (slice 1 canvas + slice 2 completion) +
                Run + results + the shipped Save/Open saved-query dialogs (federation target). The
                page's selected source is baked in; `sel` deep-linking is a follow-up. */}
            <QueryWorkbench
              ws={ws}
              source={source.name}
              sel={null}
              onSel={() => {}}
              onCreatePanel={(sql) =>
                // No dashboard is chosen here (this page isn't a dashboard) — open the panel wizard
                // under the PICK_DASHBOARD sentinel with the source + SQL prefilled; the wizard picks
                // the destination dashboard on its Save step.
                void navigate({
                  to: `/t/${encodeURIComponent(ws)}/dashboards/${encodeURIComponent(PICK_DASHBOARD)}/new-panel`,
                  search: { ds: source.name, sql },
                })
              }
            />
          </div>
        )}

        {mode === "schemas" && (
          <div className="min-h-0 min-w-0 flex-1">
            {/* Lazy: first render is what fires `dbschema.list`, so the verb only runs once the
                Schemas tab is clicked. Schemas are workspace-scoped today (not yet pinned to a
                datasource); rows open the shared designer canvas, and the designer's Migrate flow is
                where a schema is applied to this (or any) source. */}
            <Suspense
              fallback={
                <div className="flex h-full items-center justify-center bg-bg text-sm text-muted">
                  <Loader2 size={14} className="mr-2 animate-spin" /> Loading schemas…
                </div>
              }
            >
              <SchemaDesignerList
                ws={ws}
                onOpen={(name) =>
                  void navigate({
                    to: `/t/${encodeURIComponent(ws)}/schemas/${encodeURIComponent(name)}`,
                    // A fresh canvas opened from this datasource auto-imports its catalog (the user
                    // picks nothing). An existing schema already has its tables — don't re-import
                    // over it, so `from` rides only on the `new` path.
                    search: name === "new" ? { from: source.name } : {},
                  })
                }
              />
            </Suspense>
          </div>
        )}
      </div>
    </section>
  );
}

function ModeToggle({ mode, onChange }: { mode: Mode; onChange: (m: Mode) => void }) {
  return (
    <div
      className="inline-flex items-center rounded-md border border-border bg-bg p-0.5"
      role="tablist"
      aria-label="query mode"
    >
      <Button
        role="tab"
        aria-selected={mode === "discovery"}
        variant={mode === "discovery" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("discovery")}
      >
        <Eye size={13} /> Discovery
      </Button>
      <Button
        role="tab"
        aria-selected={mode === "query"}
        variant={mode === "query" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("query")}
      >
        <Database size={13} /> Query
      </Button>
      <Button
        role="tab"
        aria-selected={mode === "schemas"}
        variant={mode === "schemas" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("schemas")}
      >
        <Wand2 size={13} /> Schemas
      </Button>
    </div>
  );
}

/** The Discovery tab's List ⇄ Diagram toggle. Only shown while in Discovery mode (Query mode has its
 *  own surface). Diagram lazy-loads the React Flow ERD; List is the legacy rail + bounded preview. */
function DiscoveryViewToggle({
  view,
  onChange,
}: {
  view: DiscoveryView;
  onChange: (v: DiscoveryView) => void;
}) {
  return (
    <div
      className="inline-flex items-center rounded-md border border-border bg-bg p-0.5"
      role="tablist"
      aria-label="discovery view"
    >
      <Button
        role="tab"
        aria-selected={view === "list"}
        variant={view === "list" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("list")}
      >
        <Table2 size={13} /> List
      </Button>
      <Button
        role="tab"
        aria-selected={view === "diagram"}
        variant={view === "diagram" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("diagram")}
      >
        <Network size={13} /> Diagram
      </Button>
    </div>
  );
}

/** The route container — resolves the named source out of the shared roster hook (so probe state is
 *  consistent across list ↔ detail), renders a not-found fallback, and wires Back to the list. */
export function DatasourceDetailPage({ ws, name }: { ws: string; name: string }) {
  const { sources, probes, probe } = useDatasources();
  const navigate = useNavigate();
  const source = sources.find((s) => s.name === name);

  const back = () =>
    void navigate({ to: `/t/${encodeURIComponent(ws)}/datasources` });

  if (!source) {
    return (
      <section className="flex h-full min-w-0 flex-col bg-bg">
        <AppPageHeader
          icon={Database}
          title={name}
          description="datasource not found"
          workspace={ws}
          actions={
            <Button
              aria-label="back to datasources"
              variant="ghost"
              size="sm"
              className="gap-1.5"
              onClick={back}
            >
              <ArrowLeft size={14} /> Back
            </Button>
          }
        />
        <div className="flex h-full items-center justify-center p-8 text-center text-sm text-muted">
          <div className="max-w-sm">
            <p>
              No datasource named <code className="font-mono">{name}</code> in this workspace.
            </p>
            <p className="mt-1 text-xs">It may have been removed, or the list is still loading.</p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <DatasourceDetail
      ws={ws}
      source={source}
      probe={probes[name]}
      onTest={(n) => void probe(n)}
      onBack={back}
    />
  );
}
