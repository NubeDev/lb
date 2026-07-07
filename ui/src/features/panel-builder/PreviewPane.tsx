// The live preview pane (viz panel-editor scope: "Live preview that is the real thing"). It renders the
// DRAFT cell through the SAME `WidgetView` dispatch + the SAME `usePanelData` hook that `save` will use —
// real rows over the real bridge, fieldConfig formatting applied, the chosen view drawn. It degrades
// honestly: a denied/empty target shows the view's denied/empty state, never a fabricated value (rule 9).
//
// A TABLE-VIEW override (editor-parity step 6, toggled from the PreviewToolbar) renders the draft through
// the `table` view WITHOUT changing the saved cell (a display-only override of `view`). The preview
// subtree is wrapped in `FreezeProvider` so a frozen editor reshapes cached frames instead of re-querying
// the datasource (data-studio-ux, edit-without-requery). One responsibility: render the draft preview.

import { ChevronDown, ChevronRight } from "lucide-react";

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { WidgetView } from "@/features/dashboard/views/WidgetView";
import { FreezeProvider } from "@/features/dashboard/cache/useFreeze";

interface Props {
  /** The draft cell built from the current editor state (what save would persist). */
  cell: Cell;
  ws: string;
  scope?: VarScope;
  /** Bumps to force a re-query (the debounced edit tick). */
  refreshKey?: number;
  /** When true, render the draft through the `table` view (inspect transformed frames), not its viz. */
  tableView?: boolean;
  /** Freeze the datasource fetch — the rendered preview reshapes cached frames instead of re-querying. */
  frozen?: boolean;
  /** Collapsible mode (stacked builder): `open` + `onOpenChange` turn the "Preview" label into a
   *  disclosure — collapsed, only the header bar renders, and the reclaimed height goes to whatever
   *  flex sibling wants it (the options surface). Omit both for the always-open pane (split layout). */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
}

export function PreviewPane({
  cell,
  ws,
  scope = emptyScope(),
  refreshKey = 0,
  tableView = false,
  frozen = false,
  open = true,
  onOpenChange,
}: Props) {
  // Display-only view override: the SAVED cell is untouched; only what the preview draws changes. A cell
  // with no view of its own renders via `cellView`'s timeseries default (WidgetView), so a viewless cell
  // previews as a chart rather than "unsupported view:".
  const previewCell: Cell = { ...cell, i: "preview", ...(tableView ? { view: "table" } : {}) };
  const label = (
    <span className="text-[11px] uppercase tracking-wide text-muted">Preview{tableView ? " · table" : ""}</span>
  );
  if (!open) {
    // Collapsed: just the disclosure bar — the pane surrenders its height to the flex siblings.
    return (
      <button
        type="button"
        aria-label="preview disclosure"
        aria-expanded={false}
        className="flex w-full items-center gap-1.5 rounded-lg border border-border bg-panel px-3 py-2 text-left hover:border-fg/30"
        onClick={() => onOpenChange?.(true)}
      >
        <ChevronRight size={12} className="text-muted" />
        {label}
      </button>
    );
  }
  return (
    <div className="flex h-full min-h-[12rem] flex-col rounded-lg border border-border bg-panel p-3" aria-label="panel preview">
      <div className="mb-2 flex items-center justify-between">
        {onOpenChange ? (
          <button
            type="button"
            aria-label="preview disclosure"
            aria-expanded
            className="flex items-center gap-1.5 hover:text-fg"
            onClick={() => onOpenChange(false)}
          >
            <ChevronDown size={12} className="text-muted" />
            {label}
          </button>
        ) : (
          label
        )}
      </div>
      <div className="min-h-0 flex-1">
        <FreezeProvider value={frozen}>
          <WidgetView cell={previewCell} workspace={ws} scope={scope} refreshKey={refreshKey} />
        </FreezeProvider>
      </div>
    </div>
  );
}
