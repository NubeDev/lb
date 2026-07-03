// The live preview pane (viz panel-editor scope: "Live preview that is the real thing"). It renders the
// DRAFT cell through the SAME `WidgetView` dispatch + the SAME `usePanelData` hook that `save` will use —
// real rows over the real bridge, fieldConfig formatting applied, the chosen view drawn. It degrades
// honestly: a denied/empty target shows the view's denied/empty state, never a fabricated value (rule 9).
//
// A TABLE-VIEW toggle (editor-parity step 6) lets the author inspect the transformed frames as a table
// regardless of the chosen viz: it renders the draft through the `table` view WITHOUT changing the saved
// cell (a display-only override of `view`). One responsibility: render the draft preview.

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { Button } from "@/components/ui/button";
import { Table2 } from "lucide-react";
import { WidgetView } from "@/features/dashboard/views/WidgetView";

interface Props {
  /** The draft cell built from the current editor state (what save would persist). */
  cell: Cell;
  ws: string;
  scope?: VarScope;
  /** Bumps to force a re-query (the debounced edit tick). */
  refreshKey?: number;
  /** When true, render the draft through the `table` view (inspect transformed frames), not its viz. */
  tableView?: boolean;
  /** Toggle the table view (shows the toggle button when provided). */
  onToggleTableView?: () => void;
}

export function PreviewPane({ cell, ws, scope = emptyScope(), refreshKey = 0, tableView = false, onToggleTableView }: Props) {
  // Display-only view override: the SAVED cell is untouched; only what the preview draws changes.
  const previewCell: Cell = { ...cell, i: "preview", ...(tableView ? { view: "table" } : {}) };
  return (
    <div className="flex h-full min-h-[12rem] flex-col rounded-lg border border-border bg-panel p-3" aria-label="panel preview">
      <div className="mb-2 flex items-center justify-between">
        <span className="text-[11px] uppercase tracking-wide text-muted">Preview{tableView ? " · table" : ""}</span>
        {onToggleTableView && (
          <Button
            type="button"
            size="sm"
            variant={tableView ? "default" : "ghost"}
            aria-label="toggle table view"
            aria-pressed={tableView}
            className="h-6 px-1.5 text-[11px]"
            onClick={onToggleTableView}
          >
            <Table2 size={12} /> Table view
          </Button>
        )}
      </div>
      <div className="min-h-0 flex-1">
        <WidgetView cell={previewCell} workspace={ws} scope={scope} refreshKey={refreshKey} />
      </div>
    </div>
  );
}
