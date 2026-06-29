// GridTable — render a `RuleOutput` of kind "grid": columns + rows, with the rendered rows BOUND (a
// large grid result is capped and labelled "showing N of M" — the host already row-caps its reads, the
// page caps the render; rules-workbench scope, Risks: grid result size). One component per output kind.

interface GridTableProps {
  columns: string[];
  rows: Record<string, unknown>[];
}

/** The max rows the table renders before truncating (keeps a huge grid from melting the DOM). */
const MAX_ROWS = 100;

export function GridTable({ columns, rows }: GridTableProps) {
  const shown = rows.slice(0, MAX_ROWS);
  return (
    <div aria-label="grid result" className="rounded border border-border">
      <div className="overflow-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border text-left">
              {columns.map((c) => (
                <th key={c} className="px-3 py-2 font-medium text-fg">
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map((row, i) => (
              <tr key={i} className="border-b border-border/50">
                {columns.map((c) => (
                  <td key={c} className="px-3 py-1 font-mono">
                    {formatCell(row[c])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div aria-label="grid count" className="px-3 py-2 text-xs text-muted">
        showing {shown.length} of {rows.length}
      </div>
    </div>
  );
}

function formatCell(v: unknown): string {
  if (v === null || v === undefined) return "";
  return typeof v === "string" ? v : JSON.stringify(v);
}
