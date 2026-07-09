// The telemetry row list (telemetry-console scope): renders the snapshot/live rows newest-first, each
// row clickable on its trace_id to pivot to the correlated timeline. One component; presentation only
// (the data + pivot come from `useTelemetry`). Empty state is honest ("no events"), never fake rows.

import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import type { TelemetryRow } from "@/lib/telemetry";

interface Props {
  rows: TelemetryRow[];
  onPivotTrace: (traceId: string) => void;
}

/** A bounded, newest-first event list. The `trace_id` cell is a button → the timeline pivot. */
export function TelemetryList({ rows, onPivotTrace }: Props) {
  if (rows.length === 0) {
    return (
      <div className="py-12 text-center text-sm text-muted-foreground">
        No telemetry events match this filter.
      </div>
    );
  }
  return (
    <ul className="divide-y divide-border font-mono text-xs">
      {rows.map((row) => (
        <li key={row.seq} className="flex items-start gap-3 py-1.5" data-testid="telemetry-row">
          <LevelBadge level={row.level} />
          <OutcomeBadge outcome={row.outcome} />
          <span className="w-32 shrink-0 truncate text-muted-foreground" title={row.source}>
            {row.source || "—"}
          </span>
          <span className="w-40 shrink-0 truncate" title={row.tool}>
            {row.tool || "—"}
          </span>
          <span className="min-w-0 flex-1 truncate" title={row.msg}>
            {row.msg}
          </span>
          {row.traceId ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-auto shrink-0 px-1 py-0 font-mono text-accent underline-offset-2 hover:underline"
              onClick={() => onPivotTrace(row.traceId)}
              title="follow this trace"
            >
              {row.traceId.slice(0, 10)}
            </Button>
          ) : (
            <span className="shrink-0 text-muted-foreground">—</span>
          )}
        </li>
      ))}
    </ul>
  );
}

function LevelBadge({ level }: { level: string }) {
  // Semantic status tones route through the widened theme tokens (destructive/warning), so telemetry
  // states re-theme with the look instead of pinning fixed Tailwind palette colors.
  const tone =
    level === "error"
      ? "bg-destructive/15 text-destructive"
      : level === "warn"
        ? "bg-warning/15 text-warning"
        : "bg-muted-bg text-muted-foreground";
  return (
    <Badge variant="outline" className={`w-12 shrink-0 justify-center ${tone}`}>
      {level || "—"}
    </Badge>
  );
}

function OutcomeBadge({ outcome }: { outcome: string }) {
  if (!outcome) return <span className="w-12 shrink-0" />;
  const tone =
    outcome === "deny"
      ? "bg-destructive/15 text-destructive"
      : outcome === "error"
        ? "bg-warning/15 text-warning"
        : "bg-success/15 text-success";
  return (
    <Badge variant="outline" className={`w-14 shrink-0 justify-center ${tone}`}>
      {outcome}
    </Badge>
  );
}
