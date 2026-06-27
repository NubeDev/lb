// The Data page — the admin, READ-ONLY DB browser (data-console scope). A table picker (with row
// counts) on the left; the selected table's rows in a paged, flat grid (click a row to expand its
// full JSON); and a Grid/Graph toggle that lazy-loads the react-flow relation view. No SQL box, no
// writes — by design (the raw grid never edits; edits go through the domain verbs). Layout + wiring
// only; data lives in `useData`. This surface is admin-gated; a member never sees the nav entry.

import { Suspense, lazy, useMemo, useState } from "react";
import { Database } from "lucide-react";

import { useData } from "./useData";
import type { Row } from "@/lib/data/data.types";

// Code-split the graph (and `@xyflow/react`) so it only loads when the user flips to the graph view.
const DataGraph = lazy(() => import("./DataGraph"));

interface Props {
  ws: string;
}

type Mode = "grid" | "graph";

export function DataView({ ws }: Props) {
  const { tables, selected, rows, cursor, graph, error, select, more, loadGraph } = useData();
  const [mode, setMode] = useState<Mode>("grid");

  const showGraph = () => {
    setMode("graph");
    void loadGraph();
  };

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex items-center gap-2 border-b border-border px-4 py-3">
        <Database size={16} className="text-muted" />
        <h1 className="text-sm font-medium">Data</h1>
        {selected && (
          <div className="ml-4 flex gap-1" role="tablist" aria-label="view mode">
            <button
              role="tab"
              aria-selected={mode === "grid"}
              className={`rounded px-2 py-0.5 text-xs ${mode === "grid" ? "bg-accent/15 text-accent" : "text-muted"}`}
              onClick={() => setMode("grid")}
            >
              Grid
            </button>
            <button
              role="tab"
              aria-selected={mode === "graph"}
              className={`rounded px-2 py-0.5 text-xs ${mode === "graph" ? "bg-accent/15 text-accent" : "text-muted"}`}
              onClick={showGraph}
            >
              Graph
            </button>
          </div>
        )}
        <span className="ml-auto text-xs text-muted">{ws}</span>
      </header>

      {error && (
        <div role="alert" className="border-b border-border bg-red-500/10 px-4 py-2 text-sm text-red-400">
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        {/* Table picker */}
        <aside className="w-56 overflow-auto border-r border-border">
          <ul>
            {tables.length === 0 && <li className="px-3 py-2 text-xs text-muted">no tables</li>}
            {tables.map((t) => (
              <li key={t.table}>
                <button
                  aria-label={`select table ${t.table}`}
                  className={`flex w-full items-center justify-between px-3 py-1.5 text-left text-sm ${
                    selected === t.table ? "bg-accent/15 text-accent" : "hover:bg-panel"
                  }`}
                  onClick={() => {
                    setMode("grid");
                    void select(t.table);
                  }}
                >
                  <span>{t.table}</span>
                  <span className="text-xs text-muted tabular-nums">{t.count}</span>
                </button>
              </li>
            ))}
          </ul>
        </aside>

        {/* Grid / Graph */}
        <div className="min-w-0 flex-1 overflow-auto">
          {!selected ? (
            <div className="p-4 text-sm text-muted">Select a table to browse its rows.</div>
          ) : mode === "graph" ? (
            <Suspense fallback={<div className="p-4 text-sm text-muted">Loading graph…</div>}>
              <DataGraph graph={graph} onExpand={(id) => void loadGraph(id)} />
            </Suspense>
          ) : (
            <RowGrid rows={rows} hasMore={!!cursor} onMore={() => void more()} />
          )}
        </div>
      </div>
    </section>
  );
}

/** The flat row grid — a union of all keys across the page as columns; click a row to expand its full
 *  JSON (heterogeneous rows don't fit a fixed schema). Read-only. */
function RowGrid({ rows, hasMore, onMore }: { rows: Row[]; hasMore: boolean; onMore: () => void }) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const columns = useMemo(() => columnsOf(rows), [rows]);

  if (rows.length === 0) return <div className="p-4 text-sm text-muted">no rows</div>;

  return (
    <div className="p-2">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="text-xs text-muted">
            <th className="py-1 pr-4 font-medium">id</th>
            {columns.map((c) => (
              <th key={c} className="py-1 pr-4 font-medium">
                {c}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <Fragmentish
              key={r.id}
              row={r}
              columns={columns}
              expanded={expanded === r.id}
              onToggle={() => setExpanded(expanded === r.id ? null : r.id)}
            />
          ))}
        </tbody>
      </table>
      {hasMore && (
        <button
          aria-label="load more rows"
          className="mt-2 rounded border border-border px-3 py-1 text-xs text-muted hover:bg-panel"
          onClick={onMore}
        >
          Load more
        </button>
      )}
    </div>
  );
}

/** A row + its optional expanded-JSON detail row. Named so the table stays readable. */
function Fragmentish({
  row,
  columns,
  expanded,
  onToggle,
}: {
  row: Row;
  columns: string[];
  expanded: boolean;
  onToggle: () => void;
}) {
  return (
    <>
      <tr
        className="cursor-pointer border-t border-border/50 hover:bg-panel"
        aria-label={`row ${row.id}`}
        onClick={onToggle}
      >
        <td className="py-1 pr-4 font-mono text-xs">{row.id}</td>
        {columns.map((c) => (
          <td key={c} className="py-1 pr-4">
            {renderCell(row.data[c])}
          </td>
        ))}
      </tr>
      {expanded && (
        <tr className="bg-panel">
          <td colSpan={columns.length + 1} className="p-2">
            <pre className="overflow-auto text-xs" aria-label={`json ${row.id}`}>
              {JSON.stringify(row.data, null, 2)}
            </pre>
          </td>
        </tr>
      )}
    </>
  );
}

/** Render a single cell value — scalar verbatim, structure as compact JSON. */
function renderCell(v: unknown): string {
  if (v === null || v === undefined) return "—";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

/** The column union across the page's rows (records vary in shape; the grid infers the union). */
function columnsOf(rows: Row[]): string[] {
  const set = new Set<string>();
  for (const r of rows) for (const k of Object.keys(r.data)) set.add(k);
  return [...set];
}
