// The v3 `table` panel renderer (viz chart-types scope, Phase 2). The frame's rows in a grid, columns
// introspected from the row keys. Each NUMERIC cell is formatted through the ONE user-prefs bridge
// (`format.ts`) under that column's `fieldConfig` (a `byName` override resolves per-column unit/decimals)
// — never a local toFixed. showHeader/cellHeight/sortBy are the per-viz options; thresholds color a
// numeric cell's text. Data ONLY through `usePanelData` (invariant A); no client transform (invariant B).
// Replaces the v2 TableView's untyped, unformatted grid.
//
// One responsibility: render a table panel from a cell.

import { useMemo } from "react";

import type { Cell } from "@/lib/dashboard";
import { cellFieldConfig } from "@/lib/dashboard";
import { resolveColumns } from "@/lib/widgets";
import type { VarScope } from "@/lib/vars";
import { emptyScope } from "@/lib/vars";

import { WidgetHeader, WidgetMessage } from "../../widgets/chrome";
import { usePanelData } from "../../builder/usePanelData";
import { readTableOptions, cellHeightClass } from "./options";
import { resolveFieldOptions } from "../../fieldconfig/resolve";
import { formatValue } from "../../fieldconfig/format";
import { asNumber } from "../num";
import { RowControls, type RowControl } from "./RowControls";
import { cellTools } from "../WidgetView";

interface Props {
  cell: Cell;
  label?: string;
  scope?: VarScope;
  refreshKey?: number;
}

/** Read `options.rowControls` off the cell (the `x-lb-render` envelope field a pinned cell carries). A
 *  pinned reminder widget declares three (enable switch + run-now + delete); a hand-authored table cell
 *  has none. Absent/typed wrong → no actions column (the table renders read-only). */
function rowControlsOf(cell: Cell): RowControl[] | null {
  const rc = cell.options?.rowControls;
  if (!Array.isArray(rc) || rc.length === 0) return null;
  return rc as RowControl[];
}

export function TablePanel({ cell, label, scope = emptyScope(), refreshKey = 0 }: Props) {
  const { rows, loading, denied } = usePanelData(cell, scope, refreshKey);
  // The bridge leash for the row-control column = the cell's declared tools ∩ grant (host-enforced).
  // Computed unconditionally (hooks cannot sit after an early return — "rendered more hooks" otherwise).
  const tools = useMemo(() => cellTools(cell), [cell]);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;
  if (rows.length === 0) return <WidgetMessage tone="muted">no rows</WidgetMessage>;

  const options = readTableOptions(cell.options);
  const fc = cellFieldConfig(cell);
  // The shared column-model resolves headers through the ONE presentation resolver (`displayName`
  // override → humanize fallback), drops `hide`-marked columns, and applies `order` — identical to what
  // ResponseTable renders. Value FORMATTING (unit/decimals/thresholds) stays owned here via
  // `resolveFieldOptions`, keyed by the raw column key.
  const cols = resolveColumns(rows, fc);
  const colOpts = Object.fromEntries(
    cols.map((c) => [c.key, resolveFieldOptions(fc, { name: c.key, type: "number" })]),
  );
  const pad = cellHeightClass(options.cellHeight);

  // sortBy[0] orders the rows by a column's value (numeric when both sides are numeric, else string).
  const sorted = sortRows(rows, options.sortBy[0]);
  // The row-control column (widget-platform scope, Slice B) — when the cell carries `options.rowControls`
  // (a pinned tool-result cell does), render an actions column with the shared `<RowControls>`. Absent
  // → the table is read-only (the shipped behavior).
  const rowControls = rowControlsOf(cell);

  return (
    <div className="flex h-full flex-col" aria-label="table panel">
      <WidgetHeader label={label ?? ""} />
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full text-left text-xs">
          {options.showHeader && (
            <thead className="sticky top-0 bg-panel text-muted">
              <tr>
                {cols.map((c) => (
                  <th key={c.key} title={c.description} className={`border-b border-border px-2 font-medium ${pad}`}>
                    {c.header}
                  </th>
                ))}
                {rowControls && (
                  <th className={`border-b border-border px-2 font-medium ${pad}`}>actions</th>
                )}
              </tr>
            </thead>
          )}
          <tbody>
            {sorted.map((row, i) => (
              <tr key={i} className="odd:bg-bg/40">
                {cols.map((c) => (
                  <td key={c.key} className={`truncate border-b border-border/50 px-2 ${pad}`}>
                    {renderCell(row[c.key], colOpts[c.key])}
                  </td>
                ))}
                {rowControls && (
                  <td className={`border-b border-border/50 px-2 ${pad}`}>
                    <RowControls row={row as Record<string, unknown>} controls={rowControls} tools={tools} />
                  </td>
                )}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

/** One cell — a numeric value formatted (+ threshold-colored) through the bridge; everything else as
 *  honest text. Never a fabricated number; a non-numeric value renders verbatim. */
function renderCell(v: unknown, opts: ReturnType<typeof resolveFieldOptions>) {
  const n = asNumber(v);
  if (n === null) return v == null ? "" : typeof v === "object" ? JSON.stringify(v) : String(v);
  const f = formatValue(n, opts);
  return <span className="tabular-nums">{f.text}</span>;
}

function sortRows(
  rows: Array<Record<string, unknown>>,
  sort: { displayName: string; desc?: boolean } | undefined,
): Array<Record<string, unknown>> {
  if (!sort) return rows;
  const dir = sort.desc ? -1 : 1;
  return [...rows].sort((a, b) => {
    const av = a[sort.displayName];
    const bv = b[sort.displayName];
    const an = asNumber(av);
    const bn = asNumber(bv);
    if (an !== null && bn !== null) return (an - bn) * dir;
    return String(av ?? "").localeCompare(String(bv ?? "")) * dir;
  });
}
