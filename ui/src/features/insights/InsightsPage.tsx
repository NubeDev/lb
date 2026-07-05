// The Insights page shell — the layout frame holding the facets sidebar, the list, and the
// detail drawer (insights umbrella scope). Route: `/t/$ws/insights` (the routing scope's deep-
// linkable target). The page is a THIN LAYOUT: data + actions live in `useInsights` + the
// sibling components; this file only places them on the grid.
//
// STUB: the layout shell is real (the grid mounts the facets/list/drawer); the routing + the
// agent-dock persona pinning + the SSE subscription are TODO.

import { useState } from "react";

import type { ListQuery } from "@/lib/insights/insights.types";
import { InsightFacets } from "./InsightFacets";
import { InsightsList } from "./InsightsList";
import { InsightDetail } from "./InsightDetail";
import { useInsights } from "./useInsights";

/**
 * The Insights page. Three-pane layout: facets | list | (optional) detail drawer. The drawer
 * opens when a row is selected; closing it returns focus to the list.
 */
export function InsightsPage(): JSX.Element {
  const [filter, setFilter] = useState<ListQuery>({ limit: 50 });
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const state = useInsights(filter);

  return (
    <div className="grid h-full grid-cols-[18rem_1fr] gap-4 p-4 lg:grid-cols-[18rem_1fr_28rem]">
      <aside className="overflow-y-auto">
        <h2 className="mb-2 text-sm font-semibold uppercase text-muted-foreground">Facets</h2>
        <InsightFacets filter={filter} onChange={setFilter} />
      </aside>
      <main className="overflow-y-auto">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-sm font-semibold uppercase text-muted-foreground">Insights</h2>
          {state.loading && <span className="text-xs text-muted-foreground">Loading…</span>}
          {state.error && (
            <span className="text-xs text-destructive" role="alert">
              {state.error}
            </span>
          )}
        </div>
        <InsightsList
          items={state.items}
          selectedId={selectedId}
          onSelect={setSelectedId}
        />
      </main>
      <aside className="hidden overflow-y-auto lg:block">
        {selectedId && <InsightDetail id={selectedId} />}
      </aside>
    </div>
  );
}
