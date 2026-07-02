// The kind-tagged item renderer (channels-query-charts scope) — turns a `query` / `query_result` /
// `query_error` payload into a CARD, never raw JSON text. A `query` shows a query chip; a
// `query_result` is CHART-FIRST with a table toggle and a "Customize" affordance that opens the shared
// PlotBuilder (run the query → see the fields → decide how to plot x/y). The viewer's choice persists
// per-user via `chart_pref` and is merged over the host's auto-pick; a table-only result can still be
// plotted from scratch. RENDER + local mode/spec state only (FILE-LAYOUT).

import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, BarChart3, Database, SlidersHorizontal, Table2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { ItemPayload } from "@/lib/channel/payload.types";
import { getChartPref, setChartPref } from "@/lib/channel/chartPref.api";
import { inferFields, isPlottable, suggestFromFields, type PlotSpec } from "@/lib/charts";
import { ChartView } from "./ChartView";
import { ResultTable } from "./ResultTable";
import { chartSpecToPlotSpec } from "./toPlotSpec";
import { QueryBuilderPanel } from "./QueryBuilderPanel";

interface Props {
  payload: ItemPayload;
  /** The channel + item id key the per-viewer plot preference is stored under. Absent in previews. */
  channel?: string;
  itemId?: string;
}

/** A small SQL chip — the source + the (truncated) SQL, the durable record of "what was asked". */
function QueryChip({ source, sql }: { source: string; sql: string }) {
  return (
    <div className="flex min-w-0 items-center gap-2 text-sm" aria-label="query">
      <span className="flex shrink-0 items-center gap-1 rounded-md bg-accent/15 px-2 py-0.5 text-xs font-medium text-accent">
        <Database size={12} /> {source}
      </span>
      <code className="truncate font-mono text-xs text-muted">{sql}</code>
    </div>
  );
}

type Mode = "chart" | "table" | "customize";

export function QueryCard({ payload, channel, itemId }: Props) {
  if (payload.kind === "query") return <QueryChip source={payload.source} sql={payload.sql} />;
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
  // Agent kinds are rendered by AgentCard (MessageItem routes them there); guard so this card only
  // ever narrows to a query_result (the union is shared with the channels-agent payloads).
  if (payload.kind !== "query_result") return null;
  return <QueryResultCard payload={payload} channel={channel} itemId={itemId} />;
}

function QueryResultCard({
  payload,
  channel,
  itemId,
}: {
  payload: Extract<ItemPayload, { kind: "query_result" }>;
  channel?: string;
  itemId?: string;
}) {
  const fields = useMemo(() => inferFields(payload.rows), [payload.rows]);
  // The starting spec: the host's auto-pick when it plotted one, else a suggestion from the fields so
  // even a "table-only" result is one click from a chart.
  const hostSpec = useMemo<PlotSpec | null>(
    () => (payload.chart ? chartSpecToPlotSpec(payload.chart) : suggestFromFields(fields, payload.rows.length)),
    [payload.chart, fields, payload.rows.length],
  );

  const [spec, setSpec] = useState<PlotSpec | null>(hostSpec);
  const [mode, setMode] = useState<Mode>(hostSpec ? "chart" : "table");
  const [saving, setSaving] = useState(false);

  // Load this viewer's saved override (if any) once, and switch to the chart view when found. Guarded:
  // outside a real node transport `invoke` throws — the card still works on the host default.
  useEffect(() => {
    if (!channel || !itemId) return;
    let live = true;
    getChartPref(channel, itemId)
      .then((saved) => {
        if (live && saved) {
          setSpec(saved);
          setMode("chart");
        }
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [channel, itemId]);

  const canPlot = fields.some((f) => f.kind === "number");
  const showChart = mode === "chart" && spec && isPlottable(spec);

  const save = async (next: PlotSpec) => {
    setSpec(next);
    setMode("chart");
    if (!channel || !itemId) return;
    setSaving(true);
    try {
      await setChartPref(channel, itemId, next);
    } catch {
      /* keep the applied spec locally even if the save round-trips fail */
    } finally {
      setSaving(false);
    }
  };

  return (
    <div aria-label="query result" className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <QueryChip source={payload.source} sql={payload.sql} />
        <div className="flex shrink-0 items-center gap-1">
          {canPlot && (
            <>
              <SegButton active={mode === "chart"} onClick={() => setMode("chart")} label="Chart" icon={BarChart3} />
              <SegButton active={mode === "table"} onClick={() => setMode("table")} label="Table" icon={Table2} />
              <Button
                type="button"
                variant={mode === "customize" ? "default" : "outline"}
                size="sm"
                aria-label="customize chart"
                onClick={() => setMode("customize")}
                className="h-7 gap-1 px-2 text-xs"
              >
                <SlidersHorizontal size={13} /> Customize
              </Button>
            </>
          )}
        </div>
      </div>

      {mode === "customize" && spec ? (
        <QueryBuilderPanel
          fields={fields}
          rows={payload.rows}
          initial={spec}
          saving={saving}
          onCancel={() => setMode(showChart ? "chart" : "table")}
          onSave={save}
        />
      ) : showChart ? (
        <ChartView spec={spec!} rows={payload.rows} />
      ) : (
        <ResultTable columns={payload.columns} rows={payload.rows} truncated={payload.truncated} />
      )}
    </div>
  );
}

function SegButton({
  active,
  onClick,
  label,
  icon: Icon,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  icon: typeof BarChart3;
}) {
  return (
    <Button
      type="button"
      variant={active ? "default" : "outline"}
      size="sm"
      aria-label={label.toLowerCase()}
      aria-pressed={active}
      onClick={onClick}
      className="h-7 gap-1 px-2 text-xs"
    >
      <Icon size={13} /> {label}
    </Button>
  );
}
