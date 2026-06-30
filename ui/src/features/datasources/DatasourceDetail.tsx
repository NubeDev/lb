// The datasource detail page (datasources-ux scope) — the per-source explorer a non-SQL user lands
// on after clicking a datasource row. Three concerns, one screen:
//   1. Table discovery (left rail): click a table → see its columns, click "Preview rows" → run a
//      bounded `SELECT *` — no typing. The generated SQL is echoed read-only.
//   2. A Builder ⇄ SQL toggle. Builder is the no-SQL default (the discovery surface); SQL drops in a
//      free-form `<Textarea>` editor prefilled with the last generated query, for users who do know
//      SQL or want to tweak. Both run the SAME gated `federation.query` verb.
//   3. The results grid (right).
//
// All queries run through the real `federation.query` verb (workspace-pinned host-side, SELECT-only
// validated host + sidecar). Trusted shell code only — never an extension. One responsibility, one
// file (FILE-LAYOUT). shadcn-first per ui-standards-scope.

import { useEffect, useState } from "react";
import { useNavigate } from "@tanstack/react-router";
import {
  ArrowLeft,
  Database,
  Eye,
  Loader2,
  Play,
  Terminal,
} from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { DatasourceProbe } from "./DatasourceProbe";
import { QueryResults } from "./QueryResults";
import { TableDiscovery } from "./TableDiscovery";
import { useDatasourceQuery } from "./useDatasourceQuery";
import { useDatasources } from "./useDatasources";
import type { DatasourceSummary, ProbeResult } from "@/lib/datasources";

interface Props {
  ws: string;
  source: DatasourceSummary;
  probe?: ProbeResult;
  onTest: (name: string) => void;
  onBack: () => void;
}

type Mode = "builder" | "sql";

const PREVIEW_LIMIT = 100;

export function DatasourceDetail({ ws, source, probe, onTest, onBack }: Props) {
  const q = useDatasourceQuery(source.name);
  const [mode, setMode] = useState<Mode>("builder");
  const [selectedTable, setSelectedTable] = useState<string | null>(null);
  const [sql, setSql] = useState("");

  // Discover the table list on first mount (and when the source changes).
  useEffect(() => {
    void q.discoverTables();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [source.name, source.kind]);

  // When the builder runs a query, mirror the generated SQL into the editor so switching to SQL
  // mode shows exactly what was asked.
  useEffect(() => {
    if (q.lastSql) setSql(q.lastSql);
  }, [q.lastSql]);

  const selectTable = (table: string) => {
    setSelectedTable(table);
    void q.describeTable(table);
  };

  const preview = (table: string) => {
    setSelectedTable(table);
    void q.describeTable(table);
    void q.previewTable(table, PREVIEW_LIMIT);
  };

  const runEditor = () => {
    const trimmed = sql.trim();
    if (trimmed) void q.runSql(trimmed);
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
        {mode === "builder" && (
          <TableDiscovery
            tables={q.tables}
            selectedTable={selectedTable}
            columns={q.columns}
            loading={q.loading}
            onSelect={selectTable}
            onPreview={preview}
            onRefresh={() => void q.discoverTables()}
          />
        )}

        <div className="flex min-h-0 min-w-0 flex-1 flex-col">
          {mode === "sql" && (
            <div className="flex min-h-0 flex-1 flex-col">
              <div className="flex items-center justify-between gap-2 border-b border-border bg-panel/40 px-3 py-1.5">
                <span className="inline-flex items-center gap-1.5 text-xs text-muted">
                  <Terminal size={13} className="text-accent" /> SQL editor · SELECT only
                </span>
                <Button
                  aria-label="run sql"
                  size="sm"
                  variant="solid"
                  className="gap-1.5"
                  onClick={runEditor}
                  disabled={q.loading || !sql.trim()}
                >
                  <Play size={13} /> Run
                </Button>
              </div>
              <Textarea
                aria-label="sql editor"
                spellCheck={false}
                className="min-h-32 flex-1 resize-none rounded-none border-0 shadow-none focus-visible:ring-0"
                placeholder={`SELECT * FROM "your_table" LIMIT 100;`}
                value={sql}
                onChange={(e) => setSql(e.target.value)}
                onKeyDown={(e) => {
                  if ((e.metaKey || e.ctrlKey) && e.key === "Enter") {
                    e.preventDefault();
                    runEditor();
                  }
                }}
              />
            </div>
          )}

          <div className={mode === "sql" ? "min-h-0 flex-1 border-t border-border" : "min-h-0 flex-1"}>
            <QueryResults
              result={q.result}
              emptyHint={
                mode === "builder"
                  ? "Pick a table and preview its rows, or switch to the SQL editor."
                  : "Write a SELECT and hit Run (⌘/Ctrl+Enter)."
              }
            />
          </div>
        </div>
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
        aria-selected={mode === "builder"}
        variant={mode === "builder" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("builder")}
      >
        <Eye size={13} /> Builder
      </Button>
      <Button
        role="tab"
        aria-selected={mode === "sql"}
        variant={mode === "sql" ? "default" : "ghost"}
        size="sm"
        className="h-7 gap-1.5 px-2.5 text-xs"
        onClick={() => onChange("sql")}
      >
        <Terminal size={13} /> SQL
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
