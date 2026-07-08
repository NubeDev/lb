// WizardPreview (panel-wizard scope) — the wizard's ONE pinned render surface, with a display-only
// Chart | Table | JSON toggle so the author can sanity-check the data behind the draft without leaving
// the step. Chart is the SAME `PreviewPane`/`WidgetView` the editor uses (on the options step it renders
// through `OptionFocusPreview` so the focused option's region is emphasized); Table reuses PreviewPane's
// existing `tableView` override (the real `table` view over the same frames); JSON pretty-prints the
// rows the chart drew — the rows come DOWN from the host (the same `usePanelData` resolution feeding the
// AI-prompt rows context), never a second fetch. The toggle never touches the saved cell.
// One responsibility: render the wizard's pinned preview in one of three display modes.

import { useState } from "react";
import { Braces, ChartLine, Copy, Table2, type LucideIcon } from "lucide-react";

import type { Cell } from "@/lib/dashboard";
import { Button } from "@/components/ui/button";
import { PreviewPane } from "@/features/panel-builder/PreviewPane";
import { OptionFocusPreview } from "@/features/panel-builder/options/OptionFocusPreview";
import { optionById } from "@/features/panel-builder/options/registry";
import type { WizardStepId } from "./steps";

type PreviewMode = "chart" | "table" | "json";

const MODES: { id: PreviewMode; label: string; icon: LucideIcon }[] = [
  { id: "chart", label: "Chart", icon: ChartLine },
  { id: "table", label: "Table", icon: Table2 },
  { id: "json", label: "JSON", icon: Braces },
];

interface Props {
  cell: Cell;
  ws: string;
  refreshKey: number;
  frozen: boolean;
  step: WizardStepId;
  /** The option the options step is editing — points the chart mode's focus preview at it. */
  focusedOption?: string;
  /** The draft's resolved rows (the host's `usePanelData` result) — the JSON mode's data. */
  rows: Array<Record<string, unknown>>;
}

export function WizardPreview({ cell, ws, refreshKey, frozen, step, focusedOption, rows }: Props) {
  const [mode, setMode] = useState<PreviewMode>("chart");
  return (
    <div
      className="flex h-full min-h-[12rem] flex-col rounded-lg border border-border bg-panel p-3"
      aria-label="panel preview"
    >
      <div className="mb-2 flex items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <span className="text-[11px] uppercase tracking-wide text-muted">Preview</span>
          {mode === "chart" && step === "options" && focusedOption && (
            <span className="rounded-sm bg-accent/10 px-1.5 py-0.5 text-[11px] text-muted" aria-label="focused option">
              editing · {optionById(focusedOption)?.label ?? focusedOption}
            </span>
          )}
        </div>
        <div className="flex items-center rounded-md border border-border" role="radiogroup" aria-label="preview mode">
          {MODES.map(({ id, label, icon: Icon }) => {
            const active = mode === id;
            return (
              <Button
                key={id}
                type="button"
                role="radio"
                aria-checked={active}
                aria-label={`preview as ${id}`}
                variant={active ? "solid" : "ghost"}
                size="sm"
                className="h-6 gap-1 rounded-[5px] px-2 text-[11px]"
                onClick={() => setMode(id)}
              >
                <Icon size={11} aria-hidden /> {label}
              </Button>
            );
          })}
        </div>
      </div>

      <div className="min-h-0 flex-1">
        {mode === "chart" &&
          (step === "options" ? (
            <OptionFocusPreview
              cell={cell}
              workspace={ws}
              refreshKey={refreshKey}
              optionFocus={focusedOption ? { optionId: focusedOption } : undefined}
            />
          ) : (
            <PreviewPane cell={cell} ws={ws} refreshKey={refreshKey} frozen={frozen} />
          ))}
        {mode === "table" && <PreviewPane cell={cell} ws={ws} refreshKey={refreshKey} frozen={frozen} tableView />}
        {mode === "json" && <RowsJson rows={rows} />}
      </div>
    </div>
  );
}

/** The rows pretty-printed, with a copy affordance — a read surface, never an editor. */
function RowsJson({ rows }: { rows: Array<Record<string, unknown>> }) {
  const [copied, setCopied] = useState(false);
  if (rows.length === 0) {
    return <div className="p-3 text-xs text-muted">No rows yet — pick a source, or wait for the query.</div>;
  }
  const text = JSON.stringify(rows, null, 2);
  return (
    <div className="relative h-full">
      <div className="absolute right-1 top-1 z-10">
        <Button
          type="button"
          size="sm"
          variant="ghost"
          className="h-6 px-1.5 text-[11px]"
          onClick={() => {
            void navigator.clipboard?.writeText(text);
            setCopied(true);
            window.setTimeout(() => setCopied(false), 1200);
          }}
        >
          <Copy size={11} /> {copied ? "Copied" : "Copy"}
        </Button>
      </div>
      <pre
        className="h-full overflow-auto rounded-md border border-border bg-bg p-2 text-[11px] leading-relaxed text-fg"
        aria-label="preview rows json"
      >
        {text}
      </pre>
    </div>
  );
}
