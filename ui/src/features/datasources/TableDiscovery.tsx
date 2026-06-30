// The table + column discovery panel (datasources-ux scope) — the no-SQL "click your way in"
// affordance. A list of discovered tables (with row estimates); picking one reveals its columns and
// a one-click "Preview rows" that runs a bounded `SELECT *`. The SQL each click generates is shown
// read-only underneath so a non-SQL user sees what's being asked. One responsibility, one file.

import { ChevronRight, Database, Eye, Loader2, RefreshCw, Table2 } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { DbColumn, DbTable } from "@/lib/datasources";

interface Props {
  tables: DbTable[] | null;
  selectedTable: string | null;
  columns: DbColumn[] | null;
  loading: boolean;
  onSelect: (table: string) => void;
  onPreview: (table: string) => void;
  onRefresh: () => void;
}

export function TableDiscovery({
  tables,
  selectedTable,
  columns,
  loading,
  onSelect,
  onPreview,
  onRefresh,
}: Props) {
  return (
    <aside className="flex w-full shrink-0 flex-col border-b border-border bg-panel/30 md:w-72 md:border-b-0 md:border-r">
      <div className="flex items-center justify-between gap-2 border-b border-border px-3 py-2.5">
        <div className="flex items-center gap-2">
          <Database size={14} className="text-accent" />
          <span className="text-xs font-medium">Tables</span>
        </div>
        <Button
          aria-label="refresh tables"
          size="icon"
          variant="ghost"
          className="h-7 w-7"
          onClick={onRefresh}
        >
          <RefreshCw size={13} />
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-auto p-2">
        {loading && tables === null && (
          <div className="flex items-center gap-2 p-3 text-xs text-muted">
            <Loader2 size={13} className="animate-spin" /> Discovering tables…
          </div>
        )}
        {!loading && tables !== null && tables.length === 0 && (
          <p className="rounded-md border border-border bg-bg p-3 text-xs text-muted">
            No tables found in the <code className="font-mono">public</code> schema.
          </p>
        )}

        <ul className="space-y-0.5">
          {tables?.map((t) => (
            <TableRow
              key={t.name}
              table={t}
              active={selectedTable === t.name}
              onSelect={() => onSelect(t.name)}
            />
          ))}
        </ul>
      </div>

      {selectedTable && columns !== null && (
        <div className="max-h-[40%] shrink-0 overflow-auto border-t border-border bg-bg/60 p-2">
          <Columns
            columns={columns}
            table={selectedTable}
            onPreview={onPreview}
          />
        </div>
      )}
    </aside>
  );
}

function TableRow({
  table,
  active,
  onSelect,
}: {
  table: DbTable;
  active: boolean;
  onSelect: () => void;
}) {
  return (
    <li>
      <div
        className={`group flex items-center gap-1 rounded-md border px-2 py-1.5 text-left transition-colors ${
          active
            ? "border-accent/35 bg-accent/15 text-accent"
            : "border-transparent text-fg hover:border-border hover:bg-bg"
        }`}
      >
        <Button
          aria-label={`inspect table ${table.name}`}
          variant="ghost"
          size="sm"
          className="h-auto flex-1 justify-start gap-2 px-1.5 py-1 font-mono text-xs"
          onClick={onSelect}
        >
          <Table2 size={13} className={active ? "text-accent" : "text-muted"} />
          <span className="min-w-0 truncate">{table.name}</span>
        </Button>
        {typeof table.rows === "number" && (
          <Badge variant="outline" className="shrink-0 tabular-nums">
            {table.rows}
          </Badge>
        )}
      </div>
    </li>
  );
}

function Columns({
  columns,
  table,
  onPreview,
}: {
  columns: DbColumn[];
  table: string;
  onPreview: (table: string) => void;
}) {
  if (columns.length === 0) {
    return <p className="px-1 py-1.5 text-xs text-muted">No columns discovered.</p>;
  }
  return (
    <div className="space-y-0.5">
      <div className="px-1 pb-1 text-[11px] font-medium uppercase tracking-wide text-muted">
        Columns
      </div>
      {columns.map((c) => (
        <div key={c.name} className="flex items-center gap-2 rounded px-1 py-1 text-xs">
          <ChevronRight size={11} className="shrink-0 text-muted" />
          <span className="min-w-0 truncate font-mono">{c.name}</span>
          <Badge variant="secondary" className="ml-auto shrink-0 font-mono text-[10px]">
            {c.dataType}
          </Badge>
          {c.nullable && (
            <span className="shrink-0 text-[10px] text-muted/70" title="nullable">
              ?
            </span>
          )}
        </div>
      ))}
      <Button
        aria-label={`preview rows of ${table}`}
        variant="outline"
        size="sm"
        className="mt-2 w-full gap-1.5"
        onClick={() => onPreview(table)}
      >
        <Eye size={13} /> Preview rows
      </Button>
    </div>
  );
}
