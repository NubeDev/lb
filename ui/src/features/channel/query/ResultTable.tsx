// The result table for a `query_result` Item (channels-query-charts scope). Renders the capped
// row-set as a plain table; a `truncated` flag shows the "showing first N rows" caption. RENDER
// ONLY — columns/rows are passed in (FILE-LAYOUT).

interface Props {
  columns: string[];
  rows: Record<string, unknown>[];
  truncated?: boolean;
}

/** Render a JSON cell value as text (objects/arrays stringified, null as an em dash). */
function cell(value: unknown): string {
  if (value === null || value === undefined) return "—";
  if (typeof value === "object") return JSON.stringify(value);
  return String(value);
}

export function ResultTable({ columns, rows, truncated }: Props) {
  return (
    <div aria-label="query result table">
      <div className="max-h-72 overflow-auto rounded-md border border-border">
        <table className="w-full text-left text-xs">
          <thead className="sticky top-0 bg-panel">
            <tr>
              {columns.map((c) => (
                <th key={c} className="border-b border-border px-2 py-1 font-medium text-muted">
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, i) => (
              <tr key={i} className="odd:bg-bg even:bg-panel/40">
                {columns.map((c) => (
                  <td key={c} className="border-b border-border/60 px-2 py-1 font-mono">
                    {cell(row[c])}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {truncated && (
        <p className="mt-1 text-xs text-muted">Showing first {rows.length} rows</p>
      )}
    </div>
  );
}
