// The Library rail tab (data-studio scope) — the workspace's library panels (`panel.list`) as a
// roster-styled list (the RosterRail item look: leading icon, truncated title, trailing view badge),
// each openable in a BUILDER TAB for editing (the studio is where panels are authored now; the
// dashboard only places them). Opening loads the full spec (`panel.get`) and seeds a builder draft via
// the shipped `specToCell` bridge — saving from that tab round-trips through `panel.save` under the
// same permanent id. One responsibility: list + open. Rendered inside `StudioRail` (the rail body
// owns padding/scroll).

import { useEffect, useState } from "react";
import { Library, RefreshCw } from "lucide-react";

import { Button } from "@/components/ui/button";
import { getPanel, listPanels, specToCell, type PanelSummary } from "@/lib/panel";

interface Props {
  /** Open a builder tab on a library panel's spec (the tab name = the panel title). */
  onOpen: (panelId: string, title: string, cell: ReturnType<typeof specToCell>) => void;
  /** Bumped by the workbench when a builder tab saves a panel — re-lists so the roster stays live. */
  refreshKey?: number;
}

export function LibraryPane({ onOpen, refreshKey = 0 }: Props) {
  const [panels, setPanels] = useState<PanelSummary[]>([]);
  const [error, setError] = useState<string | undefined>();
  const [tick, setTick] = useState(0);

  useEffect(() => {
    let live = true;
    listPanels()
      .then((rows) => {
        if (!live) return;
        setPanels(rows);
        setError(undefined);
      })
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [refreshKey, tick]);

  const open = async (p: PanelSummary) => {
    try {
      const full = await getPanel(p.id.replace(/^panel:/, ""));
      onOpen(full.id, full.title, specToCell(full.id.replace(/^panel:/, ""), full.spec));
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="flex flex-col gap-1" aria-label="library panels">
      <div className="flex items-center justify-between gap-2 px-1 pb-1">
        <p className="text-xs text-muted">Open a panel to edit it in a builder tab.</p>
        <Button
          aria-label="refresh library"
          size="icon"
          variant="ghost"
          className="h-7 w-7 shrink-0"
          title="Refresh"
          onClick={() => setTick((t) => t + 1)}
        >
          <RefreshCw size={12} />
        </Button>
      </div>
      {error && (
        <p className="px-1 text-xs text-danger" role="alert">
          {error}
        </p>
      )}
      {panels.length === 0 ? (
        <p className="rounded-md border border-dashed border-border bg-bg/60 px-3 py-3 text-xs text-muted">
          No library panels yet — build one and save it.
        </p>
      ) : (
        <ul className="space-y-1">
          {panels.map((p) => (
            <li key={p.id}>
              <Button
                aria-label={`open library panel ${p.title}`}
                variant="ghost"
                onClick={() => void open(p)}
                className="h-auto w-full min-w-0 justify-start gap-2 px-2.5 py-1.5 text-left text-[13px] font-normal text-fg/90 hover:bg-fg/[0.06] hover:text-fg"
              >
                <Library size={14} className="shrink-0 text-muted" />
                <span className="min-w-0 flex-1 truncate">{p.title}</span>
                <span className="shrink-0 text-[10px] font-medium text-muted/80">{p.view}</span>
              </Button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
