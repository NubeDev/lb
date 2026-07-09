// The Insights page shell — the layout frame holding the filters toolbar, the list, and the
// detail drawer (insights umbrella scope). Route: `/t/$ws/insights` (the routing scope's deep-
// linkable target). The page is a THIN LAYOUT: data + actions live in `useInsights` + the
// sibling components; this file only places them on the grid.
//
// Voice match with the Inbox surface (collaboration scope, slice 4): the same `AppPageHeader`
// band + master–detail grid over the shadcn primitives. Insights reads as one product with
// Inbox, not a separate skin. The filters sit ABOVE the list (a normal search/table toolbar),
// not in a left rail — so the list+detail pair gets the full width.

import { useState } from "react";
import { Lightbulb, RefreshCw } from "lucide-react";

import { AppPageHeader } from "@/components/app/page-header";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import type { ListQuery } from "@/lib/insights/insights.types";
import { InsightFacets } from "./InsightFacets";
import { InsightsList } from "./InsightsList";
import { InsightDetail } from "./InsightDetail";
import { useInsights } from "./useInsights";

const INITIAL_FILTER: ListQuery = { limit: 50 };

interface Props {
  /** Workspace chip + header context. Optional so the page renders standalone in tests. */
  ws?: string;
}

/**
 * The Insights page. The filters toolbar sits above the master list; the (optional) detail
 * drawer opens on the right when a row is selected. The filter is owned by the hook (so a facet
 * change re-fetches); the toolbar drives it through `state.setFilter`.
 */
export function InsightsPage({ ws }: Props): JSX.Element {
  const [filter, setFilter] = useState<ListQuery>(INITIAL_FILTER);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const state = useInsights(INITIAL_FILTER);

  function onFilterChange(next: ListQuery) {
    setFilter(next);
    state.setFilter(next);
  }

  return (
    <section className="flex h-full flex-col bg-bg">
      <AppPageHeader
        icon={Lightbulb}
        title="Insights"
        description="Triage findings raised by rules, flows, and agents"
        workspace={ws}
        actions={
          <Button
            variant="outline"
            size="sm"
            onClick={() => void state.refresh()}
            disabled={state.loading}
            aria-label="Refresh insights"
          >
            <RefreshCw size={14} className={cn(state.loading && "animate-spin")} />
            Refresh
          </Button>
        }
      />

      {state.error && state.items.length === 0 && (
        <div className="px-4 pt-3">
          <Alert variant="destructive">
            <AlertTitle>Couldn’t load insights</AlertTitle>
            <AlertDescription>{state.error}</AlertDescription>
          </Alert>
        </div>
      )}

      <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[1fr_28rem]">
        {/* Master column: filters toolbar pinned above the list. The toolbar is the AND-filter
            the list reads (status / severity / producer ref / tag facets); a hairline divider
            separates it from the scrollable list the way a table header sits above rows. */}
        <main className="flex min-h-0 flex-col">
          <div
            aria-label="insight filters"
            className="shrink-0 border-b border-border bg-panel/40 px-4 py-3"
          >
            <InsightFacets filter={filter} onChange={onFilterChange} />
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto">
            <InsightsList
              items={state.items}
              loading={state.loading}
              selectedId={selectedId}
              onSelect={setSelectedId}
              hasMore={state.nextCursor !== null}
              onLoadMore={() => void state.loadMore()}
            />
          </div>
        </main>

        {/* Detail pane — the investigation surface (md+). Mirrors the Inbox reading pane. */}
        <aside
          aria-label="insight detail"
          className="hidden min-h-0 overflow-y-auto border-l border-border bg-panel/40 md:block"
        >
          {selectedId ? (
            <div className="p-4">
              <InsightDetail
                id={selectedId}
                onActed={() => void state.refresh()}
                onDeleted={() => {
                  // The insight is gone — close the pane and refresh the list so it drops out.
                  setSelectedId(null);
                  void state.refresh();
                }}
              />
            </div>
          ) : (
            <div className="p-4">
              <p className="text-sm text-muted">Select an insight to investigate.</p>
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}
