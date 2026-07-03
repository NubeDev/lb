// The Library dock pane (data-studio scope v2) — the workspace's library panels (`panel.list`), each
// openable in a BUILDER TAB for editing (the studio is where panels are authored now; the dashboard
// only places them). Opening loads the full spec (`panel.get`) and seeds a builder draft via the
// shipped `specToCell` bridge — saving from that tab round-trips through `panel.save` under the same
// permanent id. One responsibility: list + open.

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
    <div className="flex h-full min-h-0 flex-col gap-2 overflow-y-auto p-2" aria-label="library panels">
      <div className="flex items-center justify-between">
        <p className="text-xs text-muted">Library panels — open one to edit it in a builder tab.</p>
        <Button aria-label="refresh library" size="sm" variant="ghost" onClick={() => setTick((t) => t + 1)}>
          <RefreshCw size={12} />
        </Button>
      </div>
      {error && (
        <p className="text-xs text-red-500" role="alert">
          {error}
        </p>
      )}
      {panels.length === 0 ? (
        <p className="px-1 py-2 text-xs text-muted">No library panels yet — build one and save it.</p>
      ) : (
        <ul className="flex flex-col gap-1">
          {panels.map((p) => (
            <li key={p.id}>
              <button
                type="button"
                aria-label={`open library panel ${p.title}`}
                className="flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-xs hover:bg-accent/10"
                onClick={() => void open(p)}
              >
                <span className="flex min-w-0 items-center gap-1.5">
                  <Library size={12} className="shrink-0 text-muted" />
                  <span className="truncate">{p.title}</span>
                </span>
                <span className="ml-2 shrink-0 text-muted">{p.view}</span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
