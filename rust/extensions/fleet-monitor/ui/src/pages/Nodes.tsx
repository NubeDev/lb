import { RefreshCw, Server } from "lucide-react";

import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useSeriesFind } from "@/data/useSeriesFind";
import type { SeriesRef } from "@/data/series.types";

/** Nested child: lists the fleet's series via `series.find`. Honest loading / empty / error states. */
export function Nodes() {
  const { state, refresh } = useSeriesFind([]);
  return (
    <Card>
      <CardHeader className="flex-row items-center justify-between">
        <CardTitle>
          <Server className="h-4 w-4 text-accent" aria-hidden />
          Fleet Nodes
        </CardTitle>
        <Button variant="outline" size="sm" onClick={refresh} aria-label="Refresh nodes">
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
              <li key={s.id ?? s.name ?? i} className="py-2 text-fg" data-testid="node-row">
                {label(s)}
              </li>
            ))}
          </ul>
        )}
      </CardContent>
    </Card>
  );
}

function label(s: SeriesRef): string {
  return s.name ?? s.id ?? "(unnamed series)";
}
