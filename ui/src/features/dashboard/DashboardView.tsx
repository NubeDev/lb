// The dashboard surface (dashboard scope) — the roster + the selected dashboard's live grid of
// widgets over real series. Layout edits (add/remove/drag/resize a cell) persist through
// `dashboard.save` (the SurrealDB record, rule 4); widgets read `series.read`/`series.latest` and go
// live over the series SSE (motion, rule 3). Wiring + layout only; each piece owns its data.

import { LayoutGrid, Share2 } from "lucide-react";

import { DashboardRoster } from "./DashboardRoster";
import { Grid } from "./Grid";
import { WidgetBuilder } from "./builder/WidgetBuilder";
import { useDashboard } from "./useDashboard";
import { useSourcePicker } from "./builder/useSourcePicker";
import type { Cell, Visibility } from "@/lib/dashboard";
import type { DashboardSearch } from "@/features/routing/search";

const VISIBILITIES: Visibility[] = ["private", "team", "workspace"];

interface Props {
  ws: string;
  range?: DashboardSearch;
  onRangeChange?: (range: DashboardSearch) => void;
}

export function DashboardView({ ws, range, onRangeChange }: Props) {
  const dash = useDashboard(ws);
  const picker = useSourcePicker(ws);
  const current = dash.current;
  const copyLink = () => {
    if (typeof navigator !== "undefined" && navigator.clipboard) {
      void navigator.clipboard.writeText(window.location.href);
    }
  };

  return (
    <div className="flex h-full">
      <DashboardRoster
        roster={dash.roster}
        selectedId={current?.id ?? null}
        onSelect={dash.select}
        onCreate={dash.create}
      />

      <section className="flex min-w-0 flex-1 flex-col">
        <header className="page-header">
          <div className="page-header-icon">
            <LayoutGrid size={16} />
          </div>
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h1 className="page-title">{current?.title ?? "Dashboards"}</h1>
              {current && (
                <span className="rounded-full border border-border bg-bg px-2 py-0.5 text-[10px] uppercase text-muted">
                  {current.visibility}
                </span>
              )}
            </div>
            <p className="page-subtitle">Live workspace dashboards and series widgets.</p>
          </div>
          <div className="ml-auto flex items-center gap-2">
            {range && (
              <div className="hidden items-center gap-1 text-xs text-muted md:flex">
                <input
                  aria-label="dashboard range from"
                  className="control-field-sm w-[8.5rem]"
                  type="date"
                  value={range.from}
                  onChange={(e) => onRangeChange?.({ ...range, from: e.target.value })}
                />
                <span>to</span>
                <input
                  aria-label="dashboard range to"
                  className="control-field-sm w-[8.5rem]"
                  type="date"
                  value={range.to}
                  onChange={(e) => onRangeChange?.({ ...range, to: e.target.value })}
                />
              </div>
            )}
            {current && (
              <>
                <button
                  aria-label="copy dashboard link"
                  className="icon-button"
                  title="Copy link"
                  onClick={copyLink}
                >
                  <Share2 size={13} />
                </button>
                <select
                  aria-label="dashboard visibility"
                  className="control-field-sm"
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
                  className="danger-button-sm"
                  onClick={() => void dash.remove(current.id)}
                >
                  Delete
                </button>
              </>
            )}
            <span className="scope-pill" title={`Workspace ${ws}`}>
              <span className="h-1.5 w-1.5 rounded-full bg-accent" aria-hidden />
              <span className="truncate">{ws}</span>
            </span>
          </div>
        </header>

        {dash.error && (
          <div role="alert" className="border-b border-red-500/20 bg-red-500/10 px-4 py-2 text-sm text-red-600 dark:text-red-300">
            {dash.error}
          </div>
        )}

        {!current ? (
          <div className="flex flex-1 items-center justify-center p-6">
            <div className="flex max-w-sm flex-col items-center rounded-lg border border-dashed border-border bg-panel/70 px-6 py-7 text-center shadow-sm shadow-black/5">
              <div className="mb-3 flex h-10 w-10 items-center justify-center rounded-md border border-border bg-bg text-muted">
                <LayoutGrid size={20} />
              </div>
              <p className="text-sm font-medium text-fg">Select or create a dashboard.</p>
              <p className="mt-1 text-xs leading-5 text-muted">
                Dashboards stay scoped to the current workspace and can be shared when needed.
              </p>
            </div>
          </div>
        ) : (
          <>
            <WidgetBuilder
              ws={ws}
              existing={current.cells}
              onAdd={(cell: Cell) => void dash.saveCells([...current.cells, cell])}
            />

            <div className="min-h-0 flex-1">
              <Grid
                cells={current.cells}
                editable
                range={range}
                installed={picker.installed}
                workspace={ws}
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
