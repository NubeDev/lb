// The viz gallery (data-studio-10x scope, phase 3 stage 2) — once rows exist, one THUMBNAIL CARD per
// widget type replaces the text pill row: each chart-like card is a live mini-render of the caller's
// ACTUAL frames through the one `WidgetView`/`viz.query` path (no second renderer, no thumbnail
// engine). All cards share the draft's sources/transformations/fieldConfig, so they hit the SAME
// `vizQueryKey` cache entry — ONE query, N cheap views (the view is not part of the key). Table /
// AI widget / Template render as labeled cards (a mini-render of a template is noise — scope OQ3).
// Shape-gating mirrors `VizPicker` (`viewFitsShape`): a card the data can't honestly fill is disabled,
// not hidden. One responsibility: pick a view, visually.

import type { Cell, View } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { LayoutTemplate, Lightbulb, Sparkles, Table2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";

import { WidgetView } from "@/features/dashboard/views/WidgetView";
import { ChartBudgetProvider } from "@/features/charts";
import { type ResultShape, viewFitsShape } from "@/features/dashboard/views/shape";
import { cn } from "@/lib/utils";

/** The chart-likes that get a live mini-render, in the picker's order. */
const THUMB_VIEWS: { id: View; label: string }[] = [
  { id: "timeseries", label: "Time series" },
  { id: "barchart", label: "Bar chart" },
  { id: "stat", label: "Stat" },
  { id: "gauge", label: "Gauge" },
  { id: "bargauge", label: "Bar gauge" },
  { id: "piechart", label: "Pie chart" },
];

/** The labeled (no mini-render) cards. */
const LABEL_VIEWS: { id: View; label: string; icon: LucideIcon }[] = [
  { id: "table", label: "Table", icon: Table2 },
  { id: "genui", label: "AI widget", icon: Sparkles },
  { id: "template", label: "Template", icon: LayoutTemplate },
  { id: "insights", label: "Insights", icon: Lightbulb },
];

interface Props {
  /** The draft whose ALREADY-FETCHED frames the thumbnails render (demo-swapped when demo is on). */
  cell: Cell;
  ws: string;
  scope: VarScope;
  refreshKey: number;
  view: View;
  onChange: (view: View) => void;
  shape: ResultShape;
  /** "row" (the default strip) or "column" — the stacked builder's right-hand selection rail. */
  orientation?: "row" | "column";
}

export function VizGallery({ cell, ws, scope, refreshKey, view, onChange, shape, orientation = "row" }: Props) {
  const column = orientation === "column";
  const card = (id: View, label: string, body: React.ReactNode) => {
    const fits = viewFitsShape(id, shape);
    const selected = view === id;
    return (
      <button
        key={id}
        type="button"
        aria-label={`viz ${id}`}
        aria-pressed={selected}
        aria-disabled={!fits && !selected}
        disabled={!fits && !selected}
        title={!fits ? `the current data shape can't honestly fill a ${id}` : label}
        className={cn(
          "flex h-28 shrink-0 flex-col overflow-hidden rounded-md border text-left transition-colors",
          column ? "w-full" : "w-40",
          selected ? "border-accent shadow-[inset_0_0_0_1px_hsl(var(--accent))]" : "border-border hover:border-fg/30",
          !fits && !selected && "cursor-not-allowed border-dashed opacity-40",
        )}
        onClick={() => (fits || selected) && onChange(id)}
      >
        <div className="pointer-events-none min-h-0 flex-1 overflow-hidden bg-panel p-1">{body}</div>
        <div className={cn("border-t border-border px-2 py-1 text-[0.65rem]", selected ? "text-fg" : "text-muted")}>
          {label}
        </div>
      </button>
    );
  };

  return (
    <div
      aria-label="visualization gallery"
      className={cn(
        "flex gap-2",
        column ? "min-h-0 w-44 shrink-0 flex-col overflow-y-auto pr-1" : "shrink-0 overflow-x-auto pb-1",
      )}
    >
      {THUMB_VIEWS.map((v) =>
        card(
          v.id,
          v.label,
          // A live mini-render of the SAME cell through the one render path — only `view` differs, so
          // every thumbnail reads the one cached viz.query result (the view is not in the query key).
          // A ~160px card is worth ~120 drawn points, not the full result — without the shrunk budget
          // a big query renders 6 × N-row SVG charts and freezes the page.
          <ChartBudgetProvider budget={120}>
            <WidgetView
              cell={{ ...cell, i: `gallery-${v.id}`, view: v.id, title: "" }}
              workspace={ws}
              scope={scope}
              refreshKey={refreshKey}
            />
          </ChartBudgetProvider>,
        ),
      )}
      {LABEL_VIEWS.map((v) =>
        card(
          v.id,
          v.label,
          <div className="flex h-full items-center justify-center text-muted">
            <v.icon size={20} />
          </div>,
        ),
      )}
    </div>
  );
}
