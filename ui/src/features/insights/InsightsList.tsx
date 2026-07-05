// The insights list — the rows in the center pane. Each row shows severity (color), title,
// dedup_key, count badge, status pill, and last-ts; click selects (opens the detail drawer).
// One component per file (FILE-LAYOUT §4 frontend).
//
// STUB: the row rendering is real (reads the `Insight` shape); the severity color mapping + the
// status pill style + the keyset "load more" affordance are TODO. Today the rows render as a
// plain list with the headline fields.

import type { Insight } from "@/lib/insights/insights.types";

interface Props {
  items: Insight[];
  selectedId: string | null;
  onSelect: (id: string) => void;
}

/** Render the insights list. Newest-first (the verb already orders). */
export function InsightsList({ items, selectedId, onSelect }: Props): JSX.Element {
  if (items.length === 0) {
    return <p className="text-sm text-muted-foreground">No insights match this filter.</p>;
  }
  return (
    <ul className="divide-y divide-border rounded-md border border-border">
      {items.map((it) => (
        <li key={it.id}>
          <button
            type="button"
            onClick={() => onSelect(it.id)}
            className={`flex w-full items-start gap-3 px-3 py-2 text-left hover:bg-accent ${
              it.id === selectedId ? "bg-accent" : ""
            }`}
          >
            <SeverityDot severity={it.severity} />
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate text-sm font-medium">{it.title}</span>
                <StatusPill status={it.status} />
              </div>
              <div className="mt-0.5 flex items-center gap-2 text-xs text-muted-foreground">
                <code className="rounded bg-muted px-1">{it.dedup_key}</code>
                <span>×{it.count}</span>
                <span>·</span>
                <span>{new Date(it.last_ts).toLocaleString()}</span>
              </div>
            </div>
          </button>
        </li>
      ))}
    </ul>
  );
}

function SeverityDot({ severity }: { severity: Insight["severity"] }): JSX.Element {
  // TODO: pull colors from the theme tokens (the viz palette precedent); today a flat map.
  const color =
    severity === "critical"
      ? "bg-red-500"
      : severity === "warning"
        ? "bg-amber-500"
        : "bg-sky-500";
  return <span className={`mt-1.5 inline-block h-2 w-2 shrink-0 rounded-full ${color}`} />;
}

function StatusPill({ status }: { status: Insight["status"] }): JSX.Element {
  return (
    <span className="rounded-full border border-border px-1.5 py-0.5 text-[10px] uppercase">
      {status}
    </span>
  );
}
