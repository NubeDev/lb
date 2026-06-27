// The v2 `table` view — the source's rows in a scrollable grid (the rubix-cube InfiniteDataTable
// analog, data layer swapped to the bridge). Columns are introspected from the row keys (the
// `transformDataToColumns` idea); a denied/empty source shows an honest state. Bounded by useSource's
// BACKFILL cap — no unbounded render.

import { WidgetHeader, WidgetMessage } from "../widgets/chrome";
import { useSource } from "../builder/useSource";
import type { Source } from "@/lib/dashboard";

interface Props {
  source?: Source;
  tools: string[];
  options?: Record<string, unknown>;
  label?: string;
}

/** The union of keys across the rows, in first-seen order — the introspected columns. */
function columnsOf(rows: Array<Record<string, unknown>>): string[] {
  const seen: string[] = [];
  for (const row of rows) {
    for (const k of Object.keys(row)) if (!seen.includes(k)) seen.push(k);
  }
  return seen;
}

function cell(v: unknown): string {
  if (v == null) return "";
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

export function TableView({ source, tools, label }: Props) {
  const { rows, loading, denied } = useSource(source, tools);

  if (denied) return <WidgetMessage tone="denied">no access to this source</WidgetMessage>;
  if (loading) return <WidgetMessage tone="muted">loading…</WidgetMessage>;
  if (rows.length === 0) return <WidgetMessage tone="muted">no rows</WidgetMessage>;

  const cols = columnsOf(rows);

  return (
    <div className="flex h-full flex-col" aria-label={`table ${source?.tool ?? ""}`}>
      <WidgetHeader label={label ?? source?.tool ?? ""} />
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="w-full text-left text-xs">
          <thead className="sticky top-0 bg-panel text-muted">
            <tr>
              {cols.map((c) => (
                <th key={c} className="border-b border-border px-2 py-1 font-medium">
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, i) => (
              <tr key={i} className="odd:bg-bg/40">
                {cols.map((c) => (
                  <td key={c} className="truncate border-b border-border/50 px-2 py-1">
                    {cell(row[c])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
