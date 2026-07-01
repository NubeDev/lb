// A table with a per-row CONTROL column (channel rich responses scope) — the one interactive-list
// piece. The shipped dashboard `TablePanel` renders a read-only grid; a rich_result whose `table` view
// declares `options.rowControls` needs each row to drive a write verb (pause a reminder, run it now,
// delete it). This is the MINIMAL reuse gap: we render the table body ourselves (rows via the SAME
// shipped `usePanelData` hook — no new data path) and mount the SHIPPED SwitchControl/ButtonControl per
// row, passing the ROW OBJECT as the control's VarScope.
//
// Row-object binding (the locked decision): a control's `argsTemplate` uses `${id}`/`${enabled}` for
// ROW FIELDS (resolved from `scope.values = row` by the shipped interpolate engine, which matches
// `${name}`/`[[name]]`/`$name` — NOT `{{id}}`) and `{{value}}` for the INTERACTION value (the switch
// bool). So a pause switch is `{ id: "${id}", enabled: "{{value}}" }` and a run-now button is
// `{ id: "${id}" }`. We do NOT extend the vars engine — the shipped `interpolateArgs` already
// substitutes named scope values; we just supply the row as the scope. One responsibility: render a
// row-controlled table from a cell.

import { useMemo } from "react";

import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import { cellTools } from "@/features/dashboard/views/WidgetView";
import { usePanelData } from "@/features/dashboard/builder/usePanelData";
import { WidgetMessage } from "@/features/dashboard/widgets/chrome";
import { SwitchControl } from "@/features/dashboard/views/SwitchControl";
import { ButtonControl } from "@/features/dashboard/views/ButtonControl";

/** One per-row control declared in `options.rowControls`. `kind` picks the shipped control; `action` is
 *  the write `{ tool, argsTemplate }` (the template uses `${field}` for row fields, `{{value}}` for the
 *  interaction). `label`/`buttonLabel` are cosmetic. Mirrored by the palette when it emits the envelope. */
export interface RowControl {
  kind: "switch" | "button";
  action: { tool: string; argsTemplate?: Record<string, unknown> };
  label?: string;
  buttonLabel?: string;
}

interface Props {
  cell: Cell;
  rowControls: RowControl[];
}

/** The union of keys across the rows, in first-seen order — the introspected data columns. */
function columnsOf(rows: Array<Record<string, unknown>>): string[] {
  const seen: string[] = [];
  for (const row of rows) for (const k of Object.keys(row)) if (!seen.includes(k)) seen.push(k);
  return seen;
}

/** Render one value cell — an object as JSON, null as empty, everything else verbatim (honest text). */
function cellText(v: unknown): string {
  if (v == null) return "";
  return typeof v === "object" ? JSON.stringify(v) : String(v);
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

  const cols = columnsOf(rows);

  return (
    <div className="overflow-auto" aria-label="response table">
      <table className="w-full text-left text-xs">
        <thead className="text-muted">
          <tr>
            {cols.map((c) => (
              <th key={c} className="border-b border-border px-2 py-1 font-medium">
                {c}
              </th>
            ))}
            {rowControls.length > 0 && (
              <th className="border-b border-border px-2 py-1 font-medium">actions</th>
            )}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, i) => {
            // The row object IS the control's VarScope.values — `${id}`/`${enabled}` resolve from it.
            const scope: VarScope = { values: row as VarScope["values"], builtins: {} };
            return (
              <tr key={i} className="odd:bg-bg/40">
                {cols.map((c) => (
                  <td key={c} className="truncate border-b border-border/50 px-2 py-1">
                    {cellText(row[c])}
                  </td>
                ))}
                {rowControls.length > 0 && (
                  <td className="border-b border-border/50 px-2 py-1">
                    <div className="flex items-center gap-2">
                      {rowControls.map((rc, j) =>
                        rc.kind === "switch" ? (
                          <SwitchControl
                            key={j}
                            action={rc.action}
                            tools={tools}
                            label={rc.label ?? ""}
                            scope={scope}
                          />
                        ) : (
                          <ButtonControl
                            key={j}
                            action={rc.action}
                            tools={tools}
                            options={{ buttonLabel: rc.buttonLabel ?? rc.label ?? "Run" }}
                            label={rc.label ?? ""}
                            scope={scope}
                          />
                        ),
                      )}
                    </div>
                  </td>
                )}
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
