// The insights list — the rows in the center pane, rendered with the shadcn `Table` primitive
// (ui-standards-scope): `Table` / `TableHeader` / `TableBody` / `TableRow` / `TableHead` /
// `TableCell` from `components/ui/*`. Same data the master-list carried, now reading like a real
// ops table — a sticky header, severity dots (critical pulses), a truncated mono meta line under
// the title, and a relative time-ago. Click a row to open the detail drawer (which still shows
// occurrences). One component per file (FILE-LAYOUT §4 frontend).
//
// Tokens only — destructive/warning/accent-2 carry the severity hues, no raw `red-…`/`amber-…`
// literals. Selection rides on the `Table` row's `data-state="selected"` accent tint.

import { Lightbulb, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { cn } from "@/lib/utils";
import type { Insight, Severity, Status } from "@/lib/insights/insights.types";

interface Props {
  items: Insight[];
  /** True while a load is in flight — the empty state shows a spinner instead of resting copy. */
  loading?: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
  /** True when a next keyset page exists — renders the "Load more" affordance. */
  hasMore?: boolean;
  onLoadMore?: () => void;
}

/** Render the insights list as a shadcn `Table`. Newest-first (the verb already orders). */
export function InsightsList({
  items,
  loading,
  selectedId,
  onSelect,
  hasMore,
  onLoadMore,
}: Props): JSX.Element {
  if (items.length === 0) {
    return <EmptyPane loading={loading} />;
  }
  return (
    <div className="flex h-full flex-col">
      <div className="min-h-0 flex-1 overflow-y-auto">
        <Table>
          <TableHeader className="sticky top-0 z-10 bg-bg/80 backdrop-blur-sm">
            <TableRow className="hover:bg-transparent">
              <TableHead className="w-[8rem]">Key</TableHead>
              <TableHead>Incident / Resource</TableHead>
              <TableHead className="w-[7rem]">Severity</TableHead>
              <TableHead className="w-[7rem]">Status</TableHead>
              <TableHead className="w-[8rem] text-right">Last seen</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {items.map((it) => {
              const active = it.id === selectedId;
              const tone = severityTone(it.severity);
              return (
                <TableRow
                  key={it.id}
                  data-state={active ? "selected" : undefined}
                  aria-selected={active}
                  aria-label={`select insight ${it.dedup_key}`}
                  onClick={() => onSelect(it.id)}
                  className="cursor-pointer"
                >
                  {/* Dedup key — mono, muted, truncated. */}
                  <TableCell className="truncate font-mono text-[11px] text-muted">
                    {it.dedup_key}
                  </TableCell>

                  {/* Severity dot + title + mono meta (origin · run · count). */}
                  <TableCell>
                    <div className="flex min-w-0 items-start gap-2.5">
                      <span
                        className={cn(
                          "mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full",
                          tone.dot,
                          it.severity === "critical" && "animate-pulse",
                        )}
                        role="img"
                        aria-label={`severity: ${it.severity}`}
                      />
                      <div className="min-w-0">
                        <p className="truncate text-sm font-semibold text-fg">{it.title}</p>
                        <p className="truncate font-mono text-[11px] text-muted">
                          {it.origin.kind}:{it.origin.ref}
                          {it.origin.run ? ` · run:${it.origin.run}` : ""}
                          <span aria-hidden> · ×{it.count}</span>
                        </p>
                      </div>
                    </div>
                  </TableCell>

                  {/* Severity badge. */}
                  <TableCell>
                    <Badge
                      variant={
                        it.severity === "critical"
                          ? "destructive"
                          : it.severity === "warning"
                            ? "warning"
                            : "accent2"
                      }
                      className="text-[10px] font-bold uppercase"
                    >
                      {it.severity}
                    </Badge>
                  </TableCell>

                  {/* Status badge. */}
                  <TableCell>
                    <StatusBadge status={it.status} />
                  </TableCell>

                  {/* Time-ago. */}
                  <TableCell className="text-right font-mono text-[11px] text-muted">
                    {timeAgo(it.last_ts)}
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </div>
      {hasMore && (
        <div className="border-t border-border p-2">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            onClick={onLoadMore}
            className="w-full"
            aria-label="Load more insights"
          >
            <RefreshCw size={14} />
            Load more
          </Button>
        </div>
      )}
    </div>
  );
}

/** Severity → dot tone (matches the facet dots). */
function severityTone(s: Severity): { dot: string } {
  if (s === "critical") return { dot: "bg-destructive" };
  if (s === "warning") return { dot: "bg-warning" };
  return { dot: "bg-accent-2" };
}

/** Status as a Badge — `open` reads as the primary accent (action due), `acked` as warning
 *  (claimed), `resolved` as success (done). The shapes match the Inbox status register. */
function StatusBadge({ status }: { status: Status }): JSX.Element {
  const variant = status === "open" ? "default" : status === "acked" ? "warning" : "success";
  return (
    <Badge variant={variant} className="text-[10px] uppercase">
      {status}
    </Badge>
  );
}

/** A compact relative-time formatter ("2m ago", "1h 22m ago", "3d ago"). */
function timeAgo(ts: number): string {
  const s = Math.max(1, Math.floor((Date.now() - ts) / 1000));
  if (s < 60) return `${s}s ago`;
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 ? `${m}m ${s % 60}s ago` : `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${h}h ${m % 60}m ago` : `${h}h ago`;
  const d = Math.floor(h / 24);
  return `${d}d ago`;
}

/** The resting empty pane — mirrors the Inbox `EmptyPane`. A single quiet card; the icon spins
 *  while loading and stays still when the filter simply matches nothing. */
function EmptyPane({ loading }: { loading?: boolean }): JSX.Element {
  return (
    <div className="flex h-full items-center justify-center p-6">
      <Card className="w-full max-w-sm">
        <CardContent className="flex items-center gap-3 p-4 text-sm text-muted">
          <Lightbulb size={18} className={cn("shrink-0", loading && "animate-spin")} />
          {loading ? "Loading insights…" : "No insights match this filter."}
        </CardContent>
      </Card>
    </div>
  );
}
