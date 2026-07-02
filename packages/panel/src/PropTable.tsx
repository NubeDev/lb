// PropTable — the ce InspectPanel dense property/edge table, ported to shadcn tokens.
// A monospace, tabular-nums, hairline-separated grid: muted column headers, ellipsized
// value cells. Data-driven via columns[] + rows[] (drop ce's engine types — the host
// supplies plain cells). Widen the panel and more columns fit — the "so many options
// on resize" density the ce panel has.
//
// One responsibility: the dense table. Section wraps it; KV/Row are siblings.

import type { ReactNode } from "react";
import { cn } from "./lib/cn";

export interface PropColumn {
  /** Column key — also the header text unless `header` is given. */
  key: string;
  header?: ReactNode;
  /** Truncate + ellipsize the cell (with a title tooltip). For long value cells. */
  ellipsize?: boolean;
  /** Fixed max width in px for the cell (pairs with ellipsize). */
  maxWidth?: number;
  className?: string;
}

export interface PropRow {
  /** Stable row key. */
  id: string;
  /** Cell content per column key. */
  cells: Record<string, ReactNode>;
  /** Optional per-row emphasis (e.g. a fault/override row). */
  tone?: "default" | "warn";
}

export interface PropTableProps {
  columns: PropColumn[];
  rows: PropRow[];
  /** Shown when rows is empty. */
  empty?: ReactNode;
  className?: string;
}

/** A dense, monospace property table — the ce InspectPanel look on shadcn tokens. */
export function PropTable({ columns, rows, empty = "—", className }: PropTableProps) {
  if (rows.length === 0) {
    return <div className="py-1 font-mono text-[11px] text-lbp-muted">{empty}</div>;
  }
  return (
    <table className={cn("w-full border-collapse font-mono text-[11px] tabular-nums", className)}>
      <thead>
        <tr className="text-left text-lbp-muted">
          {columns.map((c) => (
            <th key={c.key} className="px-0 pb-1 pr-2 font-medium">
              {c.header ?? c.key}
            </th>
          ))}
        </tr>
      </thead>
      <tbody>
        {rows.map((r) => (
          <tr key={r.id} className="border-t border-lbp-border align-top">
            {columns.map((c) => {
              const v = r.cells[c.key];
              const title = c.ellipsize && typeof v === "string" ? v : undefined;
              return (
                <td
                  key={c.key}
                  title={title}
                  style={c.maxWidth ? { maxWidth: c.maxWidth } : undefined}
                  className={cn(
                    "py-[3px] pr-2 pt-[3px]",
                    c.ellipsize && "overflow-hidden text-ellipsis whitespace-nowrap",
                    r.tone === "warn" && "text-lbp-amber",
                    c.className,
                  )}
                >
                  {v ?? "—"}
                </td>
              );
            })}
          </tr>
        ))}
      </tbody>
    </table>
  );
}
