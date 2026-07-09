// The insight detail pane — the right rail. Mirrors the Inbox `DetailPane`: a `Card` with a
// `CardHeader` (title + status/severity badges + meta), a `CardContent` (origin + evidence +
// occurrences), and the ack/resolve actions in the footer (delegated to `InsightActions`).
//
// The pane is the "investigate one finding" surface; the AI dock answers "why is this firing?"
// with the page-context this pane provides. The body evidence renderer is still a JSON dump —
// a typed display (table/chart per body shape) is the named follow-up; the wrapper around it is
// now the shared shadcn register so it reads as the same product as the Inbox reading pane.

import { useEffect, useState } from "react";
import { Lightbulb, RefreshCw, Trash2 } from "lucide-react";

import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardFooter, CardHeader, CardTitle } from "@/components/ui/card";
import { cn } from "@/lib/utils";
import {
  deleteOccurrence,
  getInsight,
  listOccurrences,
} from "@/lib/insights/insights.api";
import type { Insight, OccurrencePage, Severity, Status } from "@/lib/insights/insights.types";
import { InsightActions } from "./InsightActions";

interface Props {
  id: string;
  /** Called after an ack/resolve lands so the parent list can refresh (and the pane re-opens
   *  with the new status). Optional — the pane also re-fetches its own record. */
  onActed?: () => void;
  /** Called after the whole insight is deleted so the parent can close the pane + refresh. */
  onDeleted?: () => void;
}

/** The detail pane for insight `id`. Fetches the full record + the first page of occurrences. */
export function InsightDetail({ id, onActed, onDeleted }: Props): JSX.Element {
  const [insight, setInsight] = useState<Insight | null>(null);
  const [occurrences, setOccurrences] = useState<OccurrencePage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  // Bumped after an ack/resolve (or an occurrence delete) so the pane re-fetches.
  const [version, setVersion] = useState(0);
  // The occurrence row currently being deleted (by `oseq`) — drives its spinner + disables it.
  const [deletingOcc, setDeletingOcc] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setError(null);
      setLoading(true);
      try {
        const [row, occ] = await Promise.all([
          getInsight(id),
          listOccurrences(id, undefined, 50),
        ]);
        if (cancelled) return;
        setInsight(row);
        setOccurrences(occ);
      } catch (e) {
        if (cancelled) return;
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id, version]);

  function handleActed() {
    setVersion((v) => v + 1);
    onActed?.();
  }

  async function handleDeleteOccurrence(oseq: number) {
    setDeletingOcc(oseq);
    setError(null);
    try {
      await deleteOccurrence(id, oseq);
      // Drop the row locally for instant feedback, then re-fetch to stay authoritative.
      setOccurrences((page) =>
        page ? { ...page, items: page.items.filter((o) => o.oseq !== oseq) } : page,
      );
      setVersion((v) => v + 1);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeletingOcc(null);
    }
  }

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertTitle>Couldn’t load this insight</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
      </Alert>
    );
  }
  if (loading || !insight) {
    return (
      <Card>
        <CardContent className="flex items-center gap-3 p-4 text-sm text-muted">
          <Lightbulb size={18} className="animate-spin" />
          Loading…
        </CardContent>
      </Card>
    );
  }

  return (
    <Card>
      <CardHeader>
        <div className="flex items-start justify-between gap-2">
          <CardTitle className="text-base">{insight.title}</CardTitle>
          <Badge variant="outline" className="font-mono text-[10px]">
            {insight.dedup_key}
          </Badge>
        </div>
        <div className="flex flex-wrap items-center gap-2 text-xs text-muted">
          <SeverityBadge severity={insight.severity} />
          <StatusBadge status={insight.status} />
          <span aria-hidden>·</span>
          <span>×{insight.count}</span>
          <span aria-hidden>·</span>
          <span>first {new Date(insight.first_ts).toLocaleString()}</span>
          <span aria-hidden>·</span>
          <span>last {new Date(insight.last_ts).toLocaleString()}</span>
        </div>
      </CardHeader>

      <CardContent className="space-y-4 text-sm">
        <section>
          <h4 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted">
            Origin
          </h4>
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary" className="font-mono text-[10px]">
              {insight.origin.kind}:{insight.origin.ref}
            </Badge>
            {insight.origin.run && (
              <Badge variant="outline" className="font-mono text-[10px]">
                run:{insight.origin.run}
              </Badge>
            )}
            {insight.producer && (
              <span className="text-xs text-muted">by {insight.producer}</span>
            )}
          </div>
        </section>

        {insight.body !== undefined && (
          <section>
            <h4 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted">
              Evidence
            </h4>
            {/* TODO: typed body renderer (table/chart per the body's shape); today a JSON dump. */}
            <pre className="overflow-x-auto rounded-md border border-border bg-panel-2/50 p-2 text-xs text-fg/90">
              {JSON.stringify(insight.body, null, 2)}
            </pre>
          </section>
        )}

        <section>
          <h4 className="mb-1.5 text-[11px] font-semibold uppercase tracking-wide text-muted">
            Occurrences{occurrences && ` (${occurrences.items.length} shown)`}
          </h4>
          <ul className="space-y-1.5">
            {occurrences?.items.map((o) => (
              <li
                key={o.oseq}
                className="group rounded-md border border-border bg-panel-2/30 px-2.5 py-1.5 text-xs"
              >
                <div className="flex items-center justify-between gap-2">
                  <SeverityBadge severity={o.severity} />
                  <div className="flex items-center gap-1.5">
                    <span className="text-muted">
                      {new Date(o.ts).toLocaleString()}
                    </span>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon"
                      aria-label="Delete occurrence"
                      title="Delete this occurrence"
                      onClick={() => handleDeleteOccurrence(o.oseq)}
                      disabled={deletingOcc !== null}
                      className="size-6 text-muted opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive focus-visible:opacity-100 group-hover:opacity-100"
                    >
                      {deletingOcc === o.oseq ? (
                        <RefreshCw size={12} className="animate-spin" />
                      ) : (
                        <Trash2 size={12} />
                      )}
                    </Button>
                  </div>
                </div>
                {o.data !== undefined && (
                  <pre className="mt-1.5 overflow-x-auto text-[10px] text-muted">
                    {JSON.stringify(o.data)}
                  </pre>
                )}
              </li>
            ))}
          </ul>
        </section>
      </CardContent>

      <CardFooter className="flex-col items-stretch gap-2">
        <InsightActions
          insight={insight}
          onActed={handleActed}
          onDeleted={onDeleted}
        />
      </CardFooter>
    </Card>
  );
}

function SeverityBadge({ severity }: { severity: Severity }): JSX.Element {
  const variant =
    severity === "critical" ? "destructive" : severity === "warning" ? "warning" : "accent2";
  return (
    <Badge variant={variant} className={cn("text-[10px] uppercase")}>
      {severity}
    </Badge>
  );
}

function StatusBadge({ status }: { status: Status }): JSX.Element {
  const variant = status === "open" ? "default" : status === "acked" ? "warning" : "success";
  return (
    <Badge variant={variant} className="text-[10px] uppercase">
      {status}
    </Badge>
  );
}
