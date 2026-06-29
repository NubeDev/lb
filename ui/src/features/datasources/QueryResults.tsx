// The query results grid (datasources-ux scope) — renders a `federation.query` `{columns, rows}`
// result as a flat, scrollable, typed grid. Columns come from the verb (the sidecar's column order);
// heterogeneous values render by JSON kind. Read-only. One responsibility, one file.

import { Braces } from "lucide-react";

import type { FederationQueryResult } from "@/lib/datasources";

interface Props {
  result: FederationQueryResult | null;
  emptyHint?: string;
}

export function QueryResults({ result, emptyHint }: Props) {
  if (!result) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center">
        <p className="max-w-sm text-sm text-muted">
          {emptyHint ?? "Run a query or preview a table to see rows here."}
        </p>
      </div>
    );
  }

  const { columns, rows } = result;

  if (rows.length === 0) {
    return (
      <div className="flex h-full items-center justify-center p-8 text-center">
        <p className="text-sm text-muted">The query returned no rows.</p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-w-0 flex-col">
      <div className="flex items-center gap-2 border-b border-border bg-bg px-3 py-1.5 text-xs text-muted">
        <span className="font-medium text-fg">{rows.length}</span> row
        {rows.length === 1 ? "" : "s"} · <span className="font-medium text-fg">{columns.length}</span>{" "}
        column{columns.length === 1 ? "" : "s"}
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <table className="min-w-full border-separate border-spacing-0 text-left text-sm">
          <thead className="sticky top-0 z-10 bg-panel text-xs text-muted">
            <tr>
              {columns.map((c) => (
                <th
                  key={c}
                  className="border-b border-r border-border px-3 py-2 font-medium last:border-r-0"
                >
                  <span className="font-mono">{c}</span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row, i) => (
              <ResultRow key={i} row={row} columns={columns} />
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function ResultRow({
  row,
  columns,
}: {
  row: Record<string, unknown>;
  columns: string[];
}) {
  return (
    <tr className="border-b border-border/60 transition-colors hover:bg-panel/60">
      {columns.map((c) => (
        <td
          key={c}
          className="border-b border-r border-border/60 px-3 py-1.5 align-top last:border-r-0"
        >
          <Cell value={row[c]} />
        </td>
      ))}
    </tr>
  );
}

type Kind = "string" | "number" | "boolean" | "null" | "object" | "array";

const KIND_TEXT: Record<Kind, string> = {
  string: "text-sky-700 dark:text-sky-300",
  number: "text-amber-700 dark:text-amber-300",
  boolean: "text-violet-700 dark:text-violet-300",
  null: "text-muted",
  object: "text-emerald-700 dark:text-emerald-300",
  array: "text-rose-700 dark:text-rose-300",
};

function kindOf(v: unknown): Kind {
  if (v === null || v === undefined) return "null";
  if (Array.isArray(v)) return "array";
  if (typeof v === "object") return "object";
  if (typeof v === "number") return "number";
  if (typeof v === "boolean") return "boolean";
  return "string";
}

function compact(v: unknown): string {
  if (v === undefined) return "—";
  if (v === null) return "null";
  if (typeof v === "string") return JSON.stringify(v);
  if (typeof v === "object") return JSON.stringify(v);
  return String(v);
}

function Cell({ value }: { value: unknown }) {
  const kind = kindOf(value);
  if (kind === "null") {
    return <span className="font-mono text-xs text-muted/70">null</span>;
  }
  if (kind === "object" || kind === "array") {
    return (
      <span
        className={`block max-w-[34rem] truncate font-mono text-xs leading-5 ${KIND_TEXT[kind]}`}
        title={compact(value)}
      >
        <Braces size={11} className="mr-1 inline shrink-0 opacity-70" />
        {compact(value)}
      </span>
    );
  }
  return (
    <span
      className={`block max-w-[34rem] truncate font-mono text-xs leading-5 ${KIND_TEXT[kind]}`}
      title={compact(value)}
    >
      {compact(value)}
    </span>
  );
}
