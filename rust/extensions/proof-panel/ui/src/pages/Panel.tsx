import { BadgeCheck, RefreshCw } from "lucide-react";

import { useCtx } from "@/app/useCtx";
import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useSeriesFind } from "@/data/useSeriesFind";
import type { SeriesRef } from "@/data/series.types";

/** The single page `proof-panel` contributes: it proves the federated frontend reaches real platform
 *  data through the host-mediated bridge (`series.find`). Honest loading / empty / error states — a
 *  rejected (denied / out-of-scope) call shows the error, never a fabricated list. The workspace badge
 *  proves the host `ctx` (the hard tenant wall) reached the mounted remote. */
export function Panel() {
  const { workspace } = useCtx();
  const { state, refresh } = useSeriesFind([]);
  return (
    <div className="min-h-full bg-bg p-6">
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>
            <BadgeCheck className="h-4 w-4 text-accent" aria-hidden />
            Proof Panel
            <span className="ml-2 rounded bg-border/40 px-1.5 py-0.5 text-xs font-normal text-muted">
              {workspace}
            </span>
          </CardTitle>
          <Button variant="outline" size="sm" onClick={refresh} aria-label="Refresh series">
            <RefreshCw className="h-3.5 w-3.5" aria-hidden />
            Refresh
          </Button>
        </CardHeader>
        <CardContent>
          {state.status === "loading" && <p>Loading series from the bridge…</p>}
          {state.status === "error" && (
            <p className="text-accent">Could not load series: {state.error}</p>
          )}
          {state.status === "ready" && state.data.length === 0 && (
            <p>No series in this workspace yet.</p>
          )}
          {state.status === "ready" && state.data.length > 0 && (
            <ul className="divide-y divide-border">
              {state.data.map((s, i) => (
                <li key={s.id ?? s.name ?? i} className="py-2 text-fg" data-testid="series-row">
                  {label(s)}
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

function label(s: SeriesRef): string {
  return s.name ?? s.id ?? "(unnamed series)";
}
