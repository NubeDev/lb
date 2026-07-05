// The insight detail drawer — the right pane. Shows the full body (evidence), the origin deep
// link (rule/flow/run), the occurrence ring (the per-firing evidence), and the ack/resolve
// actions. The drawer is the "investigate one finding" surface; the AI dock answers "why is this
// firing?" with the page-context this pane provides.
//
// STUB: the drawer shell + the occurrence fetch + the actions are wired; the body evidence
// renderer (typed display per `body` shape) + the origin deep-link routing + the SSE-driven live
// occurrence tail are TODO.

import { useEffect, useState } from "react";

import { getInsight, listOccurrences } from "@/lib/insights/insights.api";
import type { Insight, OccurrencePage } from "@/lib/insights/insights.types";
import { InsightActions } from "./InsightActions";

interface Props {
  id: string;
}

/** The detail drawer for insight `id`. Fetches the full record + the first page of occurrences. */
export function InsightDetail({ id }: Props): JSX.Element {
  const [insight, setInsight] = useState<Insight | null>(null);
  const [occurrences, setOccurrences] = useState<OccurrencePage | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setError(null);
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
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [id]);

  if (error) {
    return <p className="text-sm text-destructive">{error}</p>;
  }
  if (!insight) {
    return <p className="text-sm text-muted-foreground">Loading…</p>;
  }

  return (
    <article className="space-y-4">
      <header>
        <h3 className="text-base font-semibold">{insight.title}</h3>
        <p className="mt-1 text-xs text-muted-foreground">
          <code className="rounded bg-muted px-1">{insight.dedup_key}</code> · {insight.severity} ·
          ×{insight.count}
        </p>
      </header>

      <section>
        <h4 className="mb-1 text-xs font-semibold uppercase text-muted-foreground">Origin</h4>
        <p className="text-sm">
          <code className="rounded bg-muted px-1">{insight.origin.kind}:{insight.origin.ref}</code>
          {insight.origin.run && (
            <span className="ml-2 text-xs text-muted-foreground">run {insight.origin.run}</span>
          )}
        </p>
      </section>

      {insight.body !== undefined && (
        <section>
          <h4 className="mb-1 text-xs font-semibold uppercase text-muted-foreground">Evidence</h4>
          {/* TODO: typed body renderer (table/chart per the body's shape); today a JSON dump. */}
          <pre className="overflow-x-auto rounded-md bg-muted p-2 text-xs">
            {JSON.stringify(insight.body, null, 2)}
          </pre>
        </section>
      )}

      <section>
        <h4 className="mb-1 text-xs font-semibold uppercase text-muted-foreground">
          Occurrences {occurrences && `(${occurrences.items.length} shown)`}
        </h4>
        <ul className="space-y-1">
          {occurrences?.items.map((o) => (
            <li key={o.seq} className="rounded-md border border-border px-2 py-1 text-xs">
              <div className="flex items-center justify-between">
                <span>{o.severity}</span>
                <span className="text-muted-foreground">
                  {new Date(o.ts).toLocaleString()}
                </span>
              </div>
              {o.data !== undefined && (
                <pre className="mt-1 overflow-x-auto text-[10px] text-muted-foreground">
                  {JSON.stringify(o.data)}
                </pre>
              )}
            </li>
          ))}
        </ul>
      </section>

      <InsightActions insight={insight} />
    </article>
  );
}
