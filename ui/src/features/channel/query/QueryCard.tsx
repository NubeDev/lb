// The kind-tagged item renderer (channels-query-charts scope) — turns a `query` / `query_result` /
// `query_error` payload into a CARD, never raw JSON text. A `query` shows a query chip; a
// `query_result` is CHART-FIRST with a ⊞ table toggle one tap away (table-only when `chart` is
// null); a `query_error` is an inline human error. RENDER + local toggle state only (FILE-LAYOUT).

import { useState } from "react";
import { AlertTriangle, Database, Table2, BarChart3 } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { ItemPayload } from "@/lib/channel/payload.types";
import { ChartView } from "./ChartView";
import { ResultTable } from "./ResultTable";

interface Props {
  payload: ItemPayload;
}

/** A small SQL chip — the source + the (truncated) SQL, the durable record of "what was asked". */
function QueryChip({ source, sql }: { source: string; sql: string }) {
  return (
    <div className="flex items-center gap-2 text-sm" aria-label="query">
      <span className="flex items-center gap-1 rounded-md bg-accent/15 px-2 py-0.5 text-xs font-medium text-accent">
        <Database size={12} /> {source}
      </span>
      <code className="truncate font-mono text-xs text-muted">{sql}</code>
    </div>
  );
}

export function QueryCard({ payload }: Props) {
  const [showTable, setShowTable] = useState(false);

  if (payload.kind === "query") {
    return <QueryChip source={payload.source} sql={payload.sql} />;
  }

  if (payload.kind === "query_error") {
    return (
      <div role="alert" className="flex items-start gap-2 text-sm text-destructive">
        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
        <div className="min-w-0">
          <QueryChip source={payload.source} sql={payload.sql} />
          <p className="mt-1">{payload.error}</p>
        </div>
      </div>
    );
  }

  // query_result — chart-first when a chart was picked, else table-only. The toggle flips to the
  // table; with no chart the table is shown unconditionally (the toggle is hidden).
  const hasChart = payload.chart != null;
  const tableVisible = showTable || !hasChart;
  return (
    <div aria-label="query result" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <QueryChip source={payload.source} sql={payload.sql} />
        {hasChart && (
          <Button
            type="button"
            variant="outline"
            size="sm"
            aria-label={tableVisible ? "show chart" : "show table"}
            onClick={() => setShowTable((s) => !s)}
            className="h-7 px-2 text-xs"
          >
            {tableVisible ? <BarChart3 size={14} /> : <Table2 size={14} />}
          </Button>
        )}
      </div>

      {hasChart && !tableVisible && <ChartView chart={payload.chart!} rows={payload.rows} />}
      {tableVisible && (
        <ResultTable columns={payload.columns} rows={payload.rows} truncated={payload.truncated} />
      )}
    </div>
  );
}
