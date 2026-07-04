// A table with a per-row CONTROL column (channel rich responses scope) — the one interactive-list
// piece for a CHANNEL rich_result. The shipped dashboard `TablePanel` NOW renders row controls too
// (widget-platform scope, Slice B — a pinned reminder cell is interactive on the dashboard); this
// channel adapter reuses the SAME shared `<RowControls>` component so a response and its pinned cell
// render the actions column identically (the cross-surface fidelity invariant). We render the table
// body ourselves (rows via the SAME shipped `usePanelData` hook — no new data path) with simpler chrome
// than the dashboard panel (no header/sort/formatting — the channel item has its own chrome).
//
// Row-object binding (the locked decision, shared with the dashboard panel): a control's `argsTemplate`
// uses `${id}`/`${enabled}` for ROW FIELDS (resolved from `scope.values = row` by the shipped
// interpolate engine, which matches `${name}`/`[[name]]`/`$name` — NOT `{{id}}`) and `{{value}}` for the
// INTERACTION value (the switch bool). We do NOT extend the vars engine — the shipped `interpolateArgs`
// already substitutes named scope values; we just supply the row as the scope. One responsibility:
// render a row-controlled channel table from a cell.

import { useMemo } from "react";

import type { Cell } from "@/lib/dashboard";
import { cellFieldConfig } from "@/lib/dashboard";
import { resolveColumns, cellText } from "@/lib/widgets";
import { cellTools } from "@/features/dashboard/views/WidgetView";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";
import { WidgetMessage } from "@/features/dashboard/widgets/chrome";
import { RowControls, type RowControl } from "@/features/dashboard/views/table/RowControls";

// Re-export the shared `RowControl` type so the channel's existing `import type { RowControl } from
// "./ResponseTable"` callers (palette, ResponseView) keep working — the type now lives beside the shared
// renderer (`features/dashboard/views/table/RowControls.tsx`).
export type { RowControl };

interface Props {
  cell: Cell;
  rowControls: RowControl[];
}

export function ResponseTable({ cell, rowControls }: Props) {
  // Rows through the ONE shipped panel-data hook — the same read path every dashboard view uses, so the
  // table re-runs its source (e.g. `reminder.list`) exactly like a dashboard table.
  const { rows, loading, denied } = usePanelData(cell);
  // The bridge leash for EVERY control = the cell's declared tools ∩ grant (host-enforced). All row
  // controls share it so a per-row write verb (`reminder.update`/`fire`/`delete`) is forwardable.
  const tools = useMemo(() => cellTools(cell), [cell]);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;
  if (rows.length === 0) return <WidgetMessage tone="muted">no rows</WidgetMessage>;

  // The shared column-model resolves headers through the ONE presentation resolver (the cell's declared
  // `fieldConfig` → label override / humanize fallback), drops `hide`-marked columns, and applies any
  // `order`. Identical to what the read-only TablePanel renders — the only extra here is the control column.
  const cols = resolveColumns(rows, cellFieldConfig(cell));

  return (
    <div className="overflow-auto" aria-label="response table">
      <table className="w-full text-left text-xs">
        <thead className="text-muted">
          <tr>
            {cols.map((c) => (
              <th key={c.key} title={c.description} className="border-b border-border px-2 py-1 font-medium">
                {c.header}
              </th>
            ))}
            {rowControls.length > 0 && (
              <th className="border-b border-border px-2 py-1 font-medium">actions</th>
            )}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => (
            <tr key={i} className="odd:bg-bg/40">
              {cols.map((c) => (
                <td key={c.key} className="truncate border-b border-border/50 px-2 py-1">
                  {cellText(row[c.key])}
                </td>
              ))}
              {rowControls.length > 0 && (
                <td className="border-b border-border/50 px-2 py-1">
                  <RowControls
                    row={row as Record<string, unknown>}
                    controls={rowControls}
                    tools={tools}
                  />
                </td>
              )}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
