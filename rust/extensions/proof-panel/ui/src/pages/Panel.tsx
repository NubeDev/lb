import { useState } from "react";
import { Search, ShieldCheck } from "lucide-react";

import { useCtx } from "@/app/useCtx";
import { Button, Card, CardContent, CardHeader, CardTitle } from "@/components/ui";
import { useSeriesFind } from "@/data/useSeriesFind";
import { useSeriesLatest } from "@/data/useSeriesLatest";
import type { Facet } from "@/data/series.types";

/** Parse a `key:value` (or bare `key`) search box into one facet. Empty input → no facet (the host's
 *  `series.find` returns nothing for an unconstrained query, so the page prompts for a search instead
 *  of fabricating a list). */
function parseFacet(input: string): Facet[] {
  const q = input.trim();
  if (!q) return [];
  const [key, ...rest] = q.split(":");
  const value = rest.join(":").trim();
  return value ? [{ key: key.trim(), value }] : [{ key: key.trim() }];
}

/** The single page `proof-panel` contributes: it proves the federated frontend reaches REAL platform
 *  data through the host-mediated bridge. The user searches a tag facet → the page lists the matching
 *  series via `series.find`, selects one → the page shows its latest value via `series.latest`. Honest
 *  loading / empty / error states throughout — a denied (out-of-scope) call shows the error, never a
 *  fabricated list or value. The workspace badge proves the host `ctx` (the hard tenant wall) reached
 *  the mounted remote; data is reached ONLY through `bridge`, never a token, DB, or raw fetch. */
export function Panel() {
  const { workspace } = useCtx();
  const [query, setQuery] = useState("");
  const [facets, setFacets] = useState<Facet[]>([]);
  const [selected, setSelected] = useState<string | null>(null);

  const find = useSeriesFind(facets);
  const latest = useSeriesLatest(selected);

  function runSearch(e: React.FormEvent) {
    e.preventDefault();
    setSelected(null);
    setFacets(parseFacet(query));
  }

  return (
    <div className="min-h-full bg-bg p-6">
      <Card>
        <CardHeader className="flex-row items-center justify-between">
          <CardTitle>
            <ShieldCheck className="h-4 w-4 text-accent" aria-hidden />
            Proof Panel
            <span
              className="ml-2 rounded bg-border/40 px-1.5 py-0.5 text-xs font-normal text-muted"
              aria-label="workspace"
            >
              {workspace}
            </span>
          </CardTitle>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          {/* Search a tag facet (e.g. `kind:temperature`). The host's series.find intersects facets. */}
          <form aria-label="search series" onSubmit={runSearch} className="flex gap-2">
            <input
              aria-label="series facet"
              placeholder="kind:temperature"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              className="flex-1 rounded-md border border-border bg-panel px-2 py-1 text-sm text-fg outline-none focus-visible:ring-1 focus-visible:ring-accent"
            />
            <Button type="submit" size="sm" aria-label="run search">
              <Search className="h-3.5 w-3.5" aria-hidden />
              Find
            </Button>
          </form>

          {/* The series list (series.find). */}
          <section aria-label="series list">
            {find.state.status === "idle" && (
              <p className="text-muted">Search a tag facet to list this workspace&apos;s series.</p>
            )}
            {find.state.status === "loading" && <p>Loading series from the bridge…</p>}
            {find.state.status === "error" && (
              <p className="text-accent">Could not load series: {find.state.error}</p>
            )}
            {find.state.status === "ready" && find.state.data.length === 0 && (
              <p>No series match this query in this workspace.</p>
            )}
            {find.state.status === "ready" && find.state.data.length > 0 && (
              <ul className="divide-y divide-border">
                {find.state.data.map((name) => (
                  <li key={name} className="flex items-center justify-between py-2">
                    <span className="text-fg">{name}</span>
                    <Button
                      variant="outline"
                      size="sm"
                      aria-label={`select ${name}`}
                      onClick={() => setSelected(name)}
                    >
                      View latest
                    </Button>
                  </li>
                ))}
              </ul>
            )}
          </section>

          {/* The latest value of the selected series (series.latest). */}
          {selected && (
            <section aria-label="latest value" className="rounded-md border border-border p-3">
              <p className="mb-1 text-xs text-muted">
                Latest sample · <span className="text-fg">{selected}</span>
              </p>
              {latest.state.status === "loading" && <p>Reading latest…</p>}
              {latest.state.status === "error" && (
                <p className="text-accent">Could not read latest: {latest.state.error}</p>
              )}
              {latest.state.status === "ready" && latest.state.data === null && (
                <p>No samples committed for this series yet.</p>
              )}
              {latest.state.status === "ready" && latest.state.data !== null && (
                <p className="text-fg" data-testid="latest-payload">
                  {describe(latest.state.data.payload)}
                </p>
              )}
            </section>
          )}
        </CardContent>
      </Card>
    </div>
  );
}

/** Render an arbitrary JSON payload as a readable string (the platform carries heterogeneous series). */
function describe(payload: unknown): string {
  if (payload === undefined) return "(no payload)";
  if (typeof payload === "object") return JSON.stringify(payload);
  return String(payload);
}
