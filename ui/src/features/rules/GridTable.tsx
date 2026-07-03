// GridTable — render a `RuleOutput` of kind "grid": columns + rows, with the rendered rows BOUND (a
// large grid result is capped and labelled "showing N of M" — the host already row-caps its reads, the
// page caps the render; rules-workbench scope, Risks: grid result size). One component per output kind.

// A row arrives in one of TWO shapes depending on the source engine:
//   • platform (SurrealDB) rows are OBJECTS keyed by column name — `{ id: "site-001", name: … }`
//   • federation (datasource) rows are column-aligned ARRAYS — `["site-001", …]` (the sidecar
//     re-projects Arrow objects to arrays; see rust/extensions/federation/src/query.rs).
// The table must read both, else a federated query (the timescale example) renders every cell NULL —
// `row["id"]` on an array is `undefined`. `cellAt` resolves by key OR by column index accordingly.
type GridRow = Record<string, unknown> | unknown[];

interface GridTableProps {
  columns: string[];
  rows: GridRow[];
}

/** Read a cell by column, honouring both row shapes: an array row is indexed by the column's position;
 *  an object row is keyed by the column name. */
function cellAt(row: GridRow, column: string, index: number): unknown {
  return Array.isArray(row) ? row[index] : row[column];
}

/** The max rows the table renders before truncating (keeps a huge grid from melting the DOM). */
const MAX_ROWS = 100;

export function GridTable({ columns, rows }: GridTableProps) {
  const shown = rows.slice(0, MAX_ROWS);
  const truncated = rows.length > shown.length;
  return (
    <div aria-label="grid result" className="overflow-hidden rounded-md border border-border">
      <div className="max-h-[22rem] overflow-auto">
        <table className="w-full border-collapse text-sm">
          <thead className="sticky top-0 z-10">
            <tr className="text-left">
              {columns.map((c) => (
                <th
                  key={c}
                  className="border-b border-border bg-panel px-3 py-2 font-medium text-fg"
                >
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {shown.map((row, i) => (
              <tr key={i} className="odd:bg-transparent even:bg-muted/[0.06] hover:bg-accent/[0.06]">
                {columns.map((c, ci) => (
                  <td
                    key={c}
                    className="border-b border-border/40 px-3 py-1.5 font-mono text-[13px] tabular-nums text-fg"
                  >
                    {formatCell(cellAt(row, c, ci))}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div
        aria-label="grid count"
        className="flex items-center justify-between border-t border-border bg-panel/50 px-3 py-1.5 text-xs text-muted"
      >
        <span>
          {rows.length} {rows.length === 1 ? "row" : "rows"}
        </span>
        {/* Also the machine-readable "showing N of M" the gateway test asserts (and honest when capped). */}
        <span className={truncated ? "text-accent" : "text-muted"}>
          showing {shown.length} of {rows.length}
        </span>
      </div>
    </div>
  );
}

/** A NULL/empty cell renders as a dim literal, never a blank the eye skips — the value is faithful
 *  (product principle: make raw data humane without hiding it). Returns a node so NULL can be styled. */
function formatCell(v: unknown) {
  if (v === null || v === undefined)
    return <span className="italic text-muted/60">NULL</span>;
  return typeof v === "string" ? v : JSON.stringify(v);
}
