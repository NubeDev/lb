// "Add library panel" (library-panels scope, editor step d) — insert a REF cell pointing at an
// existing `panel:{id}` picked from `panel.list`. Only geometry is authored here; the spec is hydrated
// host-side on `dashboard.get`, so editing the shared panel updates every dashboard that references it.
// One responsibility: the library-panel insert affordance + its picker.

import { useEffect, useState } from "react";
import { Library } from "lucide-react";

import { Button } from "@/components/ui/button";
import type { Cell } from "@/lib/dashboard";
import { listPanels, type PanelSummary } from "@/lib/panel";
import { CAP, getSession, hasCap } from "@/lib/session";

interface Props {
  existing: Cell[];
  /** Append the ref cell (the parent persists the whole dashboard, which validates the ref resolves). */
  onAdd: (cell: Cell) => void;
}

/** A fresh cell key that doesn't collide (mirrors AddPanel's `nextKey`). */
export function nextKey(existing: Cell[]): string {
  let n = existing.length + 1;
  const keys = new Set(existing.map((c) => c.i));
  while (keys.has(`w${n}`)) n += 1;
  return `w${n}`;
}

export function AddLibraryPanel({ existing, onAdd }: Props) {
  const caps = getSession()?.caps ?? [];
  const [open, setOpen] = useState(false);
  const [panels, setPanels] = useState<PanelSummary[]>([]);

  useEffect(() => {
    if (!open) return;
    let live = true;
    listPanels()
      .then((rows) => live && setPanels(rows))
      .catch(() => live && setPanels([]));
    return () => {
      live = false;
    };
  }, [open]);

  // Needs both the list cap (to browse) and the save cap (to author the dashboard) — the host is the
  // real boundary; this is the UI lens.
  if (!hasCap(caps, CAP.panelList) || !hasCap(caps, CAP.dashboardSave)) return null;

  const add = (p: PanelSummary) => {
    const y = existing.reduce((m, c) => Math.max(m, c.y + c.h), 0);
    onAdd({
      i: nextKey(existing),
      x: 0,
      y,
      w: 8,
      h: 4,
      widget_type: "chart",
      binding: { series: "" },
      panelRef: p.id.startsWith("panel:") ? p.id : `panel:${p.id}`,
    });
    setOpen(false);
  };

  return (
    <span className="relative inline-block">
      <Button
        aria-label="add library panel"
        size="sm"
        variant="outline"
        onClick={() => setOpen((v) => !v)}
      >
        <Library size={12} /> Add library panel
      </Button>
      {open && (
        <div
          className="absolute left-0 top-9 z-20 max-h-64 w-64 overflow-y-auto rounded-md border border-border bg-panel p-1 shadow-lg"
          role="listbox"
          aria-label="library panels"
        >
          {panels.length === 0 ? (
            <div className="px-2 py-1.5 text-xs text-muted">No library panels yet.</div>
          ) : (
            panels.map((p) => (
              <button
                key={p.id}
                type="button"
                role="option"
                aria-selected="false"
                className="flex w-full items-center justify-between rounded-md px-2 py-1.5 text-left text-xs hover:bg-accent/10"
                onClick={() => add(p)}
              >
                <span className="truncate">{p.title}</span>
                <span className="ml-2 shrink-0 text-muted">{p.view}</span>
              </button>
            ))
          )}
        </div>
      )}
    </span>
  );
}
