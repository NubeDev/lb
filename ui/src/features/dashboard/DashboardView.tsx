// The dashboard surface (dashboard scope) — the roster + the selected dashboard's live grid of
// widgets over real series. Layout edits (add/remove/drag/resize a cell) persist through
// `dashboard.save` (the SurrealDB record, rule 4); widgets read `series.read`/`series.latest` and go
// live over the series SSE (motion, rule 3). Wiring + layout only; each piece owns its data.

import { LayoutGrid, Share2 } from "lucide-react";

import { DashboardRoster } from "./DashboardRoster";
import { Grid } from "./Grid";
import { AddWidget } from "./AddWidget";
import { useDashboard } from "./useDashboard";
import type { Cell, Visibility } from "@/lib/dashboard";

const VISIBILITIES: Visibility[] = ["private", "team", "workspace"];

export function DashboardView({ ws }: { ws: string }) {
  const dash = useDashboard(ws);
  const current = dash.current;

  return (
    <div className="flex h-full">
      <DashboardRoster
        roster={dash.roster}
        selectedId={current?.id ?? null}
        onSelect={dash.select}
        onCreate={dash.create}
      />

      <section className="flex min-w-0 flex-1 flex-col">
        {dash.error && (
          <div role="alert" className="border-b border-border bg-red-500/10 px-4 py-2 text-sm text-red-400">
            {dash.error}
          </div>
        )}

        {!current ? (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 text-muted">
            <LayoutGrid size={28} />
            <p className="text-sm">Select or create a dashboard.</p>
          </div>
        ) : (
          <>
            <header className="flex items-center gap-3 border-b border-border px-4 py-2">
              <h2 className="text-sm font-semibold text-fg">{current.title}</h2>
              <span className="text-[10px] uppercase text-muted">{current.visibility}</span>
              <div className="ml-auto flex items-center gap-1">
                <Share2 size={13} className="text-muted" />
                <select
                  aria-label="dashboard visibility"
                  className="rounded border border-border bg-bg px-1.5 py-0.5 text-xs"
                  value={current.visibility}
                  onChange={(e) => void dash.share(e.target.value as Visibility)}
                >
                  {VISIBILITIES.map((v) => (
                    <option key={v} value={v}>
                      {v}
                    </option>
                  ))}
                </select>
                <button
                  aria-label="delete dashboard"
                  className="rounded bg-red-500/15 px-2 py-0.5 text-xs text-red-400"
                  onClick={() => void dash.remove(current.id)}
                >
                  Delete
                </button>
              </div>
            </header>

            <AddWidget
              existing={current.cells}
              onAdd={(cell: Cell) => void dash.saveCells([...current.cells, cell])}
            />

            <div className="min-h-0 flex-1">
              <Grid
                cells={current.cells}
                editable
                onLayout={(cells) => void dash.saveCells(cells)}
                onRemove={(i) => void dash.saveCells(current.cells.filter((c) => c.i !== i))}
              />
            </div>
          </>
        )}
      </section>
    </div>
  );
}
