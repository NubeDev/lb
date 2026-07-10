// The standalone library-panel page (library-panels scope) — ONE panel rendered full-bleed on its own
// route `/t/$ws/panel/{id}`, with NO dashboard grid. It reuses the SAME shipped render path
// (`WidgetHost` → `WidgetView`/`usePanelData` → the viz bridge — no parallel renderer) and carries its
// OWN range picker + `?var-` URL selections, since there is no host dashboard to supply them. This is
// the "chart not on a dashboard" ask: a directly-linkable panel a nav entry or a shared link points at.
//
// A panel is a LENS: `panel.get` passes the three gates on the RECORD, but the panel's `sources[]`
// re-check under the VIEWER's caps at render (the shipped `viz.query` leash) — a shared panel whose
// query the viewer can't run renders "no data", never a leak. The route is cap-gated on `panel.get`.

import { useEffect, useState } from "react";
import { LineChart } from "lucide-react";

import { AppPage } from "@/components/app/page";
import { AppEmptyState } from "@/components/app/empty-state";
import { Input } from "@/components/ui/input";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { useVarScope } from "@/features/dashboard/vars/useVarScope";
import { getPanel, type Panel } from "@/lib/panel";
import type { DashboardSearch } from "@/features/routing/search";
import { PanelEmbed } from "./PanelEmbed";

interface Props {
  ws: string;
  id: string;
  range?: DashboardSearch;
  /** Update the range / `?var-` selection — one router navigate (mirrors DashboardView). */
  onSearchChange?: (search: DashboardSearch) => void;
}

/** The standalone panel surface, wrapped in the shared per-visit read cache (keyed on `ws`, like the
 *  dashboard surface) so the render path's `viz.query` de-dup + the source-picker fetch are reused. */
export function PanelPage(props: Props) {
  return (
    <DashboardCacheProvider key={props.ws} ws={props.ws}>
      <PanelPageInner {...props} />
    </DashboardCacheProvider>
  );
}

function PanelPageInner({ ws, id, range, onSearchChange }: Props) {
  const [panel, setPanel] = useState<Panel | null>(null);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    let live = true;
    setPanel(null);
    setError(undefined);
    getPanel(id)
      .then((p) => live && setPanel(p))
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [id, ws]);

  // The panel carries no dashboard variable defs; the standalone page still resolves the built-ins +
  // any `?var-` URL selections through the shipped scope (sensible defaults from the spec's own options).
  const scope = useVarScope([], range, `panel:${id}`, ws);

  return (
    <AppPage
      label="panel view"
      icon={LineChart}
      title={panel?.title ?? "Panel"}
      description="A standalone library panel — the same chart, no dashboard."
      workspace={ws}
      error={error}
      actions={
        range && (
          <div className="hidden items-center gap-1 text-xs text-muted md:flex">
            <Input
              aria-label="panel range from"
              className="h-8 w-[8.5rem] text-xs"
              type="date"
              value={range.from}
              onChange={(e) => onSearchChange?.({ ...range, from: e.target.value })}
            />
            <span>to</span>
            <Input
              aria-label="panel range to"
              className="h-8 w-[8.5rem] text-xs"
              type="date"
              value={range.to}
              onChange={(e) => onSearchChange?.({ ...range, to: e.target.value })}
            />
          </div>
        )
      }
    >
      {!panel ? (
        <AppEmptyState
          icon={LineChart}
          title={error ? "Panel not accessible." : "Loading panel…"}
          description={
            error
              ? "This panel may not exist, or you may not have access to it in this workspace."
              : "Fetching the panel definition."
          }
        />
      ) : (
        <div className="min-h-0 flex-1" data-testid="standalone-panel">
          <PanelEmbed ws={ws} id={id} spec={panel.spec} range={range} scope={scope} />
        </div>
      )}
    </AppPage>
  );
}
