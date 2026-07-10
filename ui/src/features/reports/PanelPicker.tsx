// PanelPicker — pick a panel to embed as a report panel block (reports scope). Two sections: the
// wizard's DEMO widgets (the same `timeseriesCell`/`templateCell` builders the Data→insight setup
// wizard's panel + design steps use — fully-specified v3 cells that render LIVE immediately), and the
// workspace's library panels (`listPanels()`). A library pick is hydrated CLIENT-SIDE at choose
// (`getPanel` → `specToCell`) so the preview renders live without a save+reload round-trip (a bare
// `panel:{id}` ref only hydrates server-side at `report.get`). One responsibility: choose a panel →
// a renderable cell.

import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { getPanel, listPanels, specToCell, type PanelSummary } from "@/lib/panel";
import type { Cell } from "@/lib/dashboard";
import { DEFAULT_SOURCE, DEMO_SQL, templateCell, timeseriesCell } from "@/features/admin/setup/dataToInsight";
import { TEMPLATE_GALLERY } from "@/features/admin/setup/templateGallery";

interface Props {
  ws: string;
  onPick: (cell: Cell) => void;
  onCancel: () => void;
}

/** The wizard's starter widgets, as ready-to-render cells (demo-buildings federation source). */
function demoCells(ws: string): { label: string; description: string; cell: Cell }[] {
  return [
    {
      label: "Energy over time (demo)",
      description: "Hourly average energy per site — the setup wizard's timeseries panel.",
      cell: timeseriesCell(ws, DEFAULT_SOURCE, DEMO_SQL, "Energy over time"),
    },
    ...TEMPLATE_GALLERY.filter((t) => t.id !== "ai").map((t) => ({
      label: `${t.label} (demo)`,
      description: t.description,
      cell: templateCell(ws, DEFAULT_SOURCE, t.sql, t.code, t.label),
    })),
  ];
}

export function PanelPicker({ ws, onPick, onCancel }: Props) {
  const [panels, setPanels] = useState<PanelSummary[]>([]);
  const [query, setQuery] = useState("");
  const [error, setError] = useState<string | undefined>();
  const [resolving, setResolving] = useState<string | undefined>();

  useEffect(() => {
    let live = true;
    listPanels()
      .then((p) => live && setPanels(p))
      .catch((e) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, []);

  // Hydrate the chosen library panel into a full renderable cell (title kept from the roster row).
  async function pickLibrary(p: PanelSummary) {
    setResolving(p.id);
    setError(undefined);
    try {
      const panel = await getPanel(p.id);
      onPick({ ...specToCell(p.id, panel.spec), title: p.title });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setResolving(undefined);
    }
  }

  const q = query.trim().toLowerCase();
  const rows = q ? panels.filter((p) => p.title.toLowerCase().includes(q) || p.id.includes(q)) : panels;
  const demos = demoCells(ws).filter((d) => !q || d.label.toLowerCase().includes(q));

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay/60 p-4"
      role="dialog"
      aria-label="pick a panel"
    >
      <div className="flex max-h-[70vh] w-full max-w-md flex-col rounded-lg border border-border bg-panel p-4 shadow-lg">
        <h2 className="mb-3 text-sm font-semibold">Add a panel</h2>
        <Input
          autoFocus
          aria-label="filter panels"
          className="h-8 text-xs"
          placeholder="Filter panels…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {error && <p className="mt-2 text-xs text-destructive">{error}</p>}
        <div className="mt-2 min-h-0 flex-1 overflow-auto">
          {demos.length > 0 && (
            <>
              <p className="px-1 pb-1 pt-2 text-[10px] font-semibold uppercase tracking-wide text-muted">
                Starter widgets
              </p>
              <ul className="flex flex-col gap-1">
                {demos.map((d) => (
                  <li key={d.label}>
                    <Button
                      variant="ghost"
                      className="h-auto w-full flex-col items-start gap-0.5 p-2 text-left text-xs"
                      onClick={() => onPick(d.cell)}
                    >
                      <span className="font-medium">{d.label}</span>
                      <span className="line-clamp-1 text-[11px] font-normal text-muted">{d.description}</span>
                    </Button>
                  </li>
                ))}
              </ul>
            </>
          )}
          <p className="px-1 pb-1 pt-3 text-[10px] font-semibold uppercase tracking-wide text-muted">
            Library panels
          </p>
          {rows.length === 0 ? (
            <p className="p-4 text-center text-xs text-muted">
              {error ? "Panels unavailable." : "No library panels — create one on a dashboard first."}
            </p>
          ) : (
            <ul className="flex flex-col gap-1">
              {rows.map((p) => (
                <li key={p.id}>
                  <Button
                    variant="ghost"
                    className="h-auto w-full justify-start truncate p-2 text-xs"
                    disabled={resolving === p.id}
                    onClick={() => void pickLibrary(p)}
                  >
                    <span className="truncate font-medium">{resolving === p.id ? "Loading…" : p.title}</span>
                    <span className="ml-2 truncate font-mono text-muted">{p.id}</span>
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
        <div className="mt-3 flex justify-end">
          <Button variant="ghost" size="sm" onClick={onCancel}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}
