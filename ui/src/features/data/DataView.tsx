// The Data page — the admin, READ-ONLY DB browser (data-console scope). A table picker (with row
// counts) on the left; the selected table's rows in a paged, typed grid (click a row to expand its
// full JSON); and a Grid/Graph toggle that lazy-loads the react-flow relation view. No SQL box, no
// writes — by design (the raw grid never edits; edits go through the domain verbs). Layout + wiring
// only; data lives in `useData`. This surface is admin-gated; a member never sees the nav entry.

import { Suspense, lazy, useMemo, useState } from "react";
import { Braces, ChevronRight, Database, Network, Table2 } from "lucide-react";

import { useData } from "./useData";
import type { Row, TableCount } from "@/lib/data/data.types";

// Code-split the graph (and `@xyflow/react`) so it only loads when the user flips to the graph view.
const DataGraph = lazy(() => import("./DataGraph"));

interface Props {
  ws: string;
}

type Mode = "grid" | "graph";
type ValueKind = "string" | "number" | "boolean" | "null" | "object" | "array";

const TYPE_STYLES: Record<
  ValueKind,
  { label: string; dot: string; chip: string; text: string; json: string }
> = {
  string: {
    label: "string",
    dot: "bg-sky-500",
    chip: "border-sky-500/25 bg-sky-500/10",
    text: "text-sky-700 dark:text-sky-300",
    json: "text-sky-700 dark:text-sky-300",
  },
  number: {
    label: "number",
    dot: "bg-amber-500",
    chip: "border-amber-500/25 bg-amber-500/10",
    text: "text-amber-700 dark:text-amber-300",
    json: "text-amber-700 dark:text-amber-300",
  },
  boolean: {
    label: "bool",
    dot: "bg-violet-500",
    chip: "border-violet-500/25 bg-violet-500/10",
    text: "text-violet-700 dark:text-violet-300",
    json: "text-violet-700 dark:text-violet-300",
  },
  null: {
    label: "null",
    dot: "bg-muted",
    chip: "border-border bg-panel",
    text: "text-muted",
    json: "text-muted",
  },
  object: {
    label: "object",
    dot: "bg-emerald-500",
    chip: "border-emerald-500/25 bg-emerald-500/10",
    text: "text-emerald-700 dark:text-emerald-300",
    json: "text-emerald-700 dark:text-emerald-300",
  },
  array: {
    label: "array",
    dot: "bg-rose-500",
    chip: "border-rose-500/25 bg-rose-500/10",
    text: "text-rose-700 dark:text-rose-300",
    json: "text-rose-700 dark:text-rose-300",
  },
};

export function DataView({ ws }: Props) {
  const { tables, selected, rows, cursor, graph, error, select, more, loadGraph } = useData();
  const [mode, setMode] = useState<Mode>("grid");
  const selectedTable = tables.find((t) => t.table === selected);

  const showGraph = () => {
    setMode("graph");
    void loadGraph();
  };

  return (
    <section className="flex h-full flex-col bg-bg">
      <header className="flex min-h-[3.75rem] items-center gap-3 border-b border-border bg-panel/55 px-4 py-2.5">
        <div className="flex h-8 w-8 items-center justify-center rounded-md border border-border bg-bg">
          <Database size={16} className="text-accent" />
        </div>
        <div className="min-w-0">
          <div className="flex items-center gap-2">
            <h1 className="text-sm font-semibold">Data</h1>
            <span className="rounded border border-border bg-bg px-1.5 py-0.5 text-[11px] text-muted">
              read-only
            </span>
          </div>
          <p className="mt-0.5 truncate text-xs text-muted">
            Browse raw workspace records, inspect JSON, and follow relation edges.
          </p>
        </div>

        {selected && (
          <div className="ml-2 hidden items-center gap-2 text-xs text-muted md:flex">
            <Metric label="table" value={selected} mono />
            <Metric label="rows" value={String(selectedTable?.count ?? rows.length)} />
          </div>
        )}

        {selected && (
          <div
            className="ml-auto flex rounded-md border border-border bg-bg p-0.5"
            role="tablist"
            aria-label="view mode"
          >
            <ModeTab mode="grid" active={mode === "grid"} onClick={() => setMode("grid")} />
            <ModeTab mode="graph" active={mode === "graph"} onClick={showGraph} />
          </div>
        )}
        {!selected && <span className="ml-auto text-xs text-muted">{ws}</span>}
      </header>

      {error && (
        <div
          role="alert"
          className="border-b border-border bg-red-500/10 px-4 py-2 text-sm text-red-700 dark:text-red-300"
        >
          {error}
        </div>
      )}

      <div className="flex min-h-0 flex-1">
        <TablePicker
          tables={tables}
          selected={selected}
          onSelect={(table) => {
            setMode("grid");
            void select(table);
          }}
        />

        <div className="min-w-0 flex-1 overflow-hidden">
          {!selected ? (
            <EmptySelection tables={tables.length} />
          ) : mode === "graph" ? (
            <Suspense fallback={<PanelMessage title="Loading graph" body="Reading relation edges." />}>
              <DataGraph graph={graph} onExpand={(id) => void loadGraph(id)} />
            </Suspense>
          ) : (
            <RowGrid
              table={selected}
              rows={rows}
              hasMore={!!cursor}
              onMore={() => void more()}
            />
          )}
        </div>
      </div>
    </section>
  );
}

function ModeTab({
  mode,
  active,
  onClick,
}: {
  mode: Mode;
  active: boolean;
  onClick: () => void;
}) {
  const Icon = mode === "grid" ? Table2 : Network;
  return (
    <button
      role="tab"
      aria-selected={active}
      className={`inline-flex h-7 items-center gap-1.5 rounded px-2.5 text-xs transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent ${
        active ? "bg-accent/15 text-accent" : "text-muted hover:bg-panel hover:text-fg"
      }`}
      onClick={onClick}
    >
      <Icon size={14} />
      {mode === "grid" ? "Grid" : "Graph"}
    </button>
  );
}

function Metric({ label, value, mono = false }: { label: string; value: string; mono?: boolean }) {
  return (
    <span className="inline-flex items-baseline gap-1 rounded border border-border bg-bg px-2 py-1">
      <span>{label}</span>
      <span className={mono ? "font-mono text-fg" : "font-medium tabular-nums text-fg"}>
        {value}
      </span>
    </span>
  );
}

function TablePicker({
  tables,
  selected,
  onSelect,
}: {
  tables: TableCount[];
  selected: string | null;
  onSelect: (table: string) => void;
}) {
  const totalRows = tables.reduce((sum, t) => sum + t.count, 0);

  return (
    <aside className="flex w-64 shrink-0 flex-col border-r border-border bg-panel/35">
      <div className="border-b border-border px-3 py-2.5">
        <div className="flex items-center justify-between gap-2">
          <div className="text-xs font-medium text-fg">Tables</div>
          <div className="text-[11px] tabular-nums text-muted">
            {tables.length} / {totalRows} rows
          </div>
        </div>
      </div>

      <ul className="flex-1 overflow-auto p-2">
        {tables.length === 0 && (
          <li className="rounded-md border border-border bg-bg p-3 text-xs text-muted">
            No tables found.
          </li>
        )}
        {tables.map((t) => {
          const active = selected === t.table;
          return (
            <li key={t.table} className="mb-1 last:mb-0">
              <button
                aria-label={`select table ${t.table}`}
                className={`group flex w-full items-center justify-between gap-2 rounded-md border px-2.5 py-2 text-left text-sm transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent ${
                  active
                    ? "border-accent/35 bg-accent/15 text-accent"
                    : "border-transparent text-fg hover:border-border hover:bg-bg"
                }`}
                onClick={() => onSelect(t.table)}
              >
                <span className="min-w-0 truncate font-mono text-xs">{t.table}</span>
                <span
                  className={`rounded px-1.5 py-0.5 text-[11px] tabular-nums ${
                    active ? "bg-bg/70 text-accent" : "bg-bg text-muted group-hover:text-fg"
                  }`}
                >
                  {t.count}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
    </aside>
  );
}

function EmptySelection({ tables }: { tables: number }) {
  return (
    <div className="flex h-full items-center justify-center p-8">
      <div className="max-w-sm text-center">
        <div className="mx-auto flex h-10 w-10 items-center justify-center rounded-md border border-border bg-panel">
          <Table2 size={18} className="text-accent" />
        </div>
        <h2 className="mt-3 text-sm font-medium">Select a table</h2>
        <p className="mt-1 text-sm text-muted">
          {tables === 0
            ? "This workspace has no raw store tables yet."
            : "Choose a table to inspect rows, expand full JSON, or open the relation graph."}
        </p>
      </div>
    </div>
  );
}

function PanelMessage({ title, body }: { title: string; body: string }) {
  return (
    <div className="flex h-full items-center justify-center p-8 text-center">
      <div>
        <div className="text-sm font-medium">{title}</div>
        <div className="mt-1 text-xs text-muted">{body}</div>
      </div>
    </div>
  );
}

/** The flat row grid — a union of all keys across the page as columns; click a row to expand its full
 *  JSON (heterogeneous rows don't fit a fixed schema). Read-only. */
function RowGrid({
  table,
  rows,
  hasMore,
  onMore,
}: {
  table: string;
  rows: Row[];
  hasMore: boolean;
  onMore: () => void;
}) {
  const [expanded, setExpanded] = useState<string | null>(null);
  const columns = useMemo(() => columnsOf(rows), [rows]);

  if (rows.length === 0) {
    return <PanelMessage title={`No rows in ${table}`} body="Rows will appear here after writes." />;
  }

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex min-h-[3.25rem] flex-wrap items-center gap-2 border-b border-border bg-bg px-3 py-2">
        <div className="min-w-0">
          <div className="truncate font-mono text-sm font-medium">{table}</div>
          <div className="text-xs text-muted">
            {rows.length} loaded row{rows.length === 1 ? "" : "s"} · {columns.length} inferred
            column{columns.length === 1 ? "" : "s"}
          </div>
        </div>
        <TypeLegend />
      </div>

      <div className="min-h-0 flex-1 overflow-auto">
        <table className="min-w-full border-separate border-spacing-0 text-left text-sm">
          <thead className="sticky top-0 z-10 bg-panel text-xs text-muted shadow-[0_1px_0_hsl(var(--border))]">
            <tr>
              <th className="w-[22rem] border-b border-r border-border px-3 py-2 font-medium">id</th>
              {columns.map((c) => (
                <th key={c} className="border-b border-r border-border px-3 py-2 font-medium last:border-r-0">
                  <span className="font-mono">{c}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <DataRow
                key={r.id}
                row={r}
                columns={columns}
                expanded={expanded === r.id}
                onToggle={() => setExpanded(expanded === r.id ? null : r.id)}
              />
            ))}
          </tbody>
        </table>
      </div>

      {hasMore && (
        <div className="border-t border-border bg-panel/45 px-3 py-2">
          <button
            type="button"
            aria-label="load more rows"
            className="rounded-md border border-border bg-bg px-3 py-1.5 text-xs text-fg transition-colors hover:bg-panel focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent"
            onClick={onMore}
          >
            Load more rows
          </button>
        </div>
      )}
    </div>
  );
}

/** A row + its optional expanded-JSON detail row. */
function DataRow({
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
        className="cursor-pointer border-b border-border/60 transition-colors hover:bg-panel/70"
        aria-label={`row ${row.id}`}
        onClick={onToggle}
      >
        <td className="border-b border-r border-border/60 px-3 py-1.5 align-top">
          <div className="flex max-w-[22rem] items-start gap-2">
            <ChevronRight
              size={14}
              className={`mt-0.5 shrink-0 text-muted transition-transform ${expanded ? "rotate-90" : ""}`}
              aria-hidden
            />
            <span className="min-w-0 truncate font-mono text-xs text-fg">{row.id}</span>
          </div>
        </td>
        {columns.map((c) => (
          <td key={c} className="border-b border-r border-border/60 px-3 py-1.5 align-top last:border-r-0">
            <CellValue value={row.data[c]} />
          </td>
        ))}
      </tr>
      {expanded && (
        <tr className="bg-panel/60">
          <td colSpan={columns.length + 1} className="border-b border-border p-0">
            <div className="border-y border-border bg-bg/70 px-3 py-2">
              <div className="mb-2 flex items-center gap-2 text-xs text-muted">
                <Braces size={14} className="text-accent" />
                <span>Full JSON</span>
                <span className="font-mono text-fg">{row.id}</span>
              </div>
              <pre
                className="max-h-[28rem] overflow-auto rounded-md border border-border bg-panel p-3 text-xs leading-5"
                aria-label={`json ${row.id}`}
              >
                <JsonValue value={row.data} />
              </pre>
            </div>
          </td>
        </tr>
      )}
    </>
  );
}

function TypeLegend() {
  const kinds: ValueKind[] = ["string", "number", "boolean", "object", "array", "null"];

  return (
    <div className="ml-auto flex flex-wrap items-center gap-1.5" aria-label="data type colors">
      {kinds.map((kind) => {
        const style = TYPE_STYLES[kind];
        return (
          <span key={kind} className={`inline-flex items-center gap-1 text-[11px] ${style.text}`}>
            <span className={`h-1.5 w-1.5 rounded-full ${style.dot}`} aria-hidden />
            {style.label}
          </span>
        );
      })}
    </div>
  );
}

function CellValue({ value }: { value: unknown }) {
  const kind = valueKind(value);
  const style = TYPE_STYLES[kind];

  if (value === undefined) {
    return <span className="text-muted">-</span>;
  }

  return (
    <span
      className={`block max-w-[34rem] truncate font-mono text-xs leading-5 ${style.text}`}
      title={`${style.label}: ${stringifyCompact(value)}`}
    >
      {stringifyCompact(value)}
    </span>
  );
}

function TypeToken({ kind }: { kind: ValueKind }) {
  const style = TYPE_STYLES[kind];
  return <span className={`${style.json}`}>{style.label}</span>;
}

function JsonValue({ value, depth = 0 }: { value: unknown; depth?: number }) {
  const kind = valueKind(value);

  if (kind === "array") {
    const items = value as unknown[];
    if (items.length === 0) return <span className={TYPE_STYLES.array.json}>[]</span>;
    return (
      <>
        <span className={TYPE_STYLES.array.json}>[</span>
        {items.map((item, index) => (
          <span key={index}>
            {"\n"}
            {indent(depth + 1)}
            <JsonValue value={item} depth={depth + 1} />
            {index < items.length - 1 && <span className="text-muted">,</span>}
          </span>
        ))}
        {"\n"}
        {indent(depth)}
        <span className={TYPE_STYLES.array.json}>]</span>
      </>
    );
  }

  if (kind === "object") {
    const entries = Object.entries(value as Record<string, unknown>);
    if (entries.length === 0) return <span className={TYPE_STYLES.object.json}>{"{}"}</span>;
    return (
      <>
        <span className={TYPE_STYLES.object.json}>{"{"}</span>
        {entries.map(([key, item], index) => (
          <span key={key}>
            {"\n"}
            {indent(depth + 1)}
            <span className="text-fg">"{key}"</span>
            <span className="text-muted">: </span>
            <JsonValue value={item} depth={depth + 1} />
            {index < entries.length - 1 && <span className="text-muted">,</span>}
          </span>
        ))}
        {"\n"}
        {indent(depth)}
        <span className={TYPE_STYLES.object.json}>{"}"}</span>
      </>
    );
  }

  if (kind === "string") {
    return <span className={TYPE_STYLES.string.json}>{JSON.stringify(value)}</span>;
  }
  if (kind === "number") {
    return <span className={TYPE_STYLES.number.json}>{String(value)}</span>;
  }
  if (kind === "boolean") {
    return <span className={TYPE_STYLES.boolean.json}>{String(value)}</span>;
  }
  return <TypeToken kind="null" />;
}

function indent(depth: number) {
  return "  ".repeat(depth);
}

function valueKind(value: unknown): ValueKind {
  if (value === null || value === undefined) return "null";
  if (Array.isArray(value)) return "array";
  if (typeof value === "object") return "object";
  if (typeof value === "number") return "number";
  if (typeof value === "boolean") return "boolean";
  return "string";
}

function stringifyCompact(value: unknown): string {
  if (value === undefined) return "-";
  if (value === null) return "null";
  if (typeof value === "string") return JSON.stringify(value);
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

/** The column union across the page's rows (records vary in shape; the grid infers the union). */
function columnsOf(rows: Row[]): string[] {
  const set = new Set<string>();
  for (const r of rows) for (const k of Object.keys(r.data)) set.add(k);
  return [...set];
}
