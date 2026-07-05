// The insights list — the rows in the center pane. Mirrors the Inbox master-list voice: a
// scrollable `<ul>` of `<li>` rows, each a button with an active-state left rail (`border-l-2`),
// an accent severity dot, a truncated title, and quiet meta beneath. The data marks unique to
// insights (severity tone, status badge, dedup key, count) layer onto that shared shape — same
// product, different signals. One component per file (FILE-LAYOUT §4 frontend).

import { Lightbulb, RefreshCw } from "lucide-react";

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
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

/** Render the insights list. Newest-first (the verb already orders). */
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
      <ul role="list" className="min-h-0 flex-1 overflow-y-auto">
        {items.map((it) => {
          const active = it.id === selectedId;
          return (
            <li key={it.id} role="listitem" className="border-b border-border last:border-b-0">
              <button
                type="button"
                onClick={() => onSelect(it.id)}
                aria-current={active ? "true" : undefined}
                className={cn(
                  "flex w-full items-start gap-3 border-l-2 px-4 py-3 text-left transition-colors",
                  active
                    ? "border-l-accent bg-accent/10"
                    : "border-l-transparent hover:bg-panel",
                )}
              >
                <SeverityDot severity={it.severity} />
                <span className="min-w-0 flex-1">
                  <span className="flex items-center gap-2">
                    <span className="truncate text-sm font-medium text-fg">{it.title}</span>
                    <StatusBadge status={it.status} />
                  </span>
                  <span className="mt-1 flex flex-wrap items-center gap-2 text-xs text-muted">
                    <Badge variant="secondary" className="font-mono text-[10px]">
                      {it.dedup_key}
                    </Badge>
                    <span>×{it.count}</span>
                    <span aria-hidden>·</span>
                    <span>{new Date(it.last_ts).toLocaleString()}</span>
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ul>
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

/** Severity as a colored dot — `destructive`/`warning`/`accent-2` follow the theme tokens (the
 *  widened palette), not raw Tailwind hues, so dark/light both read right. */
function SeverityDot({ severity }: { severity: Severity }): JSX.Element {
  const tone =
    severity === "critical"
      ? "bg-destructive"
      : severity === "warning"
        ? "bg-warning"
        : "bg-accent-2";
  return (
    <span
      className={cn("mt-1.5 h-2 w-2 shrink-0 rounded-full", tone)}
      role="img"
      aria-label={`severity: ${severity}`}
    />
  );
}

/** Status as a Badge — `open` reads as the primary accent (action due), `acked` as warning
 *  (claimed), `resolved` as success (done). The shapes match the Inbox status register. */
function StatusBadge({ status }: { status: Status }): JSX.Element {
  const variant = status === "open" ? "default" : status === "acked" ? "warning" : "success";
  return (
    <Badge variant={variant} className="ml-auto shrink-0 text-[10px] uppercase">
      {status}
    </Badge>
  );
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
