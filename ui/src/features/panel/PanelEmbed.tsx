// PanelEmbed — the reusable "one panel rendered outside a grid" primitive (library-panels + reports
// scopes). It is the render CORE extracted from `PanelPage`: `DashboardCacheProvider` (keyed on `ws`)
// → a renderable `Cell` → `WidgetHost` (the ONE shipped widget path — no parallel renderer). Every
// standalone panel surface shares this file: `PanelPage` (the route), report panel blocks, and future
// channel embeds.
//
// The provider is REQUIRED, not optional: `WidgetHost` → `WidgetView`/`useDatasourceList` read the
// per-visit cache it supplies, and break without it (the known gotcha). Wrapping here means every
// consumer gets it for free.
//
// Two input modes (exactly one required):
//   - `cell` — a ready {@link Cell} (an inline spec or a hydrated `panel:{id}` ref). Rendered directly.
//   - `id`   — a library panel id; fetched via `getPanel` → `specToCell` (with optional `spec` to skip
//              the fetch when the caller already holds the spec). The lens story holds: `panel.get`
//              gates the record; the panel's sources re-check under the viewer's caps at render.

import { useEffect, useState } from "react";
import { EyeOff } from "lucide-react";

import { WidgetPlaceholder } from "@/features/dashboard/WidgetPlaceholder";
import { WidgetHost } from "@/features/dashboard/WidgetHost";
import { DashboardCacheProvider } from "@/features/dashboard/cache/DashboardQueryProvider";
import { specToCell, getPanel, type PanelSpec } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
import type { VarScope } from "@/lib/vars";
import type { DashboardSearch } from "@/features/routing/search";

interface Props {
  ws: string;
  /** A library panel id — fetched (or built from `spec`) into a renderable cell. */
  id?: string;
  /** An already-known panel spec for `id` — skips the `getPanel` fetch. */
  spec?: PanelSpec;
  /** A ready cell (inline spec or hydrated ref) — rendered directly, no fetch. */
  cell?: Cell;
  range?: DashboardSearch;
  scope?: VarScope;
}

/** Render one panel outside any grid, wrapped in the required per-visit read cache (keyed on `ws`). */
export function PanelEmbed({ ws, id, spec, cell, range, scope }: Props) {
  return (
    <DashboardCacheProvider key={ws} ws={ws}>
      <PanelEmbedInner ws={ws} id={id} spec={spec} cell={cell} range={range} scope={scope} />
    </DashboardCacheProvider>
  );
}

function PanelEmbedInner({ ws, id, spec, cell, range, scope }: Props) {
  // A ready cell (or a spec we can turn into one) renders immediately; only a bare `id` needs a fetch.
  const seed = cell ?? (id && spec ? specToCell(id, spec) : null);
  const [resolved, setResolved] = useState<Cell | null>(seed);
  const [error, setError] = useState<string | undefined>();

  useEffect(() => {
    // Only fetch when we were given just an id (no cell, no spec).
    if (cell || spec || !id) {
      setResolved(cell ?? (id && spec ? specToCell(id, spec) : null));
      setError(undefined);
      return;
    }
    let live = true;
    setResolved(null);
    setError(undefined);
    getPanel(id)
      .then((p) => live && setResolved(specToCell(id, p.spec)))
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [id, spec, cell, ws]);

  if (error) {
    return (
      <WidgetPlaceholder
        icon={EyeOff}
        title="Panel not accessible"
        detail="This panel may not exist, or you may not have access to it in this workspace."
        testId="panel-embed-error"
      />
    );
  }
  if (!resolved) {
    return (
      <WidgetPlaceholder icon={EyeOff} title="Loading panel…" detail="Fetching the panel definition." />
    );
  }
  return (
    <div className="min-h-0 flex-1" data-testid="panel-embed">
      <WidgetHost cell={resolved} range={range} workspace={ws} scope={scope} />
    </div>
  );
}
