// The Plot (X/Y axes) tab — the dashboard half of "run the query, see the fields, decide how to plot".
// It runs the DRAFT panel's query through the ONE data hook (`usePanelData` — invariant A, the same rows
// the preview + save use), types the returned fields, and mounts the SHARED `PlotBuilder` so the author
// picks the chart type + x/y/series with a live preview. The choice persists as `options.plot` (a
// `PlotSpec`), which the cartesian panels render through the shared `PlotChart`. One editor, one builder,
// one renderer across the dashboard and the channel.
//
// One responsibility: edit `state.options.plot` against the draft query's live fields.

import { useMemo } from "react";

import { Button } from "@/components/ui/button";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";
import { inferFields, readPlotSpec, suggestFromFields, type PlotSpec } from "@/lib/charts";
import { PlotBuilder } from "@/features/charts";

import type { EditorState } from "@/lib/panel-kit/cellEditorState";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";

interface Props {
  /** The draft cell (what save would persist) — its query supplies the fields to plot against. */
  draft: Cell;
  state: EditorState;
  patch: (next: Partial<EditorState>) => void;
  scope?: VarScope;
  refreshKey?: number;
  /** Forwarded to `PlotBuilder` — a host with its own pinned preview (the wizard) passes `false`. */
  preview?: boolean;
}

export function PlotAxesTab({ draft, state, patch, scope = emptyScope(), refreshKey = 0, preview = true }: Props) {
  const { rows, loading, denied } = usePanelData(draft, scope, refreshKey);
  const fields = useMemo(() => inferFields(rows), [rows]);

  const saved = readPlotSpec((state.options as Record<string, unknown>).plot);
  const spec = saved ?? suggestFromFields(fields, rows.length);

  const setSpec = (next: PlotSpec) => patch({ options: { ...state.options, plot: next } });
  const reset = () => {
    const { plot: _drop, ...rest } = state.options as Record<string, unknown>;
    void _drop;
    patch({ options: rest });
  };

  if (denied) return <Note>No access to this source — grant the query’s capability to plot it.</Note>;
  if (loading) return <Note>Running the query…</Note>;
  if (fields.length === 0) return <Note>Run a query on the Query tab first — then pick how to plot its fields here.</Note>;
  if (!spec) return <Note>This result has no numeric field to plot. Add a numeric column to the query.</Note>;

  return (
    <div className="flex flex-col gap-3 py-1" aria-label="plot axes tab">
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs text-muted">
          Pick the chart type and assign the fields to the X and Y axes. The panel saves and renders this.
        </p>
        {saved && (
          <Button type="button" variant="ghost" size="sm" onClick={reset} className="h-7 shrink-0 px-2 text-xs">
            Reset to auto
          </Button>
        )}
      </div>
      <PlotBuilder fields={fields} rows={rows} spec={spec} onChange={setSpec} preview={preview} />
    </div>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return (
    <div className="py-3 text-xs text-muted" aria-label="plot axes tab">
      {children}
    </div>
  );
}
