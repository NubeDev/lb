// The workbench's "+ Open view" header menu (data-studio-10x scope, phase 2) — lists "New panel"
// plus the pages-as-panes registry, filtered to the surfaces the caller's route gating (`allowed`)
// already grants (the same UI lens as the nav; the gateway re-checks server-side). An already-open
// view kind shows as "focus" — one pane per kind in the first cut. One responsibility: the menu.

import { useEffect, useRef, useState } from "react";
import { Plus } from "lucide-react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

import { VIEW_PANES } from "./workbenchPanes";

interface Props {
  /** The caller's granted surfaces (`allowed`) — ungranted view kinds are omitted, not disabled. */
  allowed: string[];
  /** Whether a view kind already has an open pane (the item re-activates instead of duplicating). */
  isOpen: (kind: string) => boolean;
  onOpenView: (kind: string) => void;
  onNewPanel: () => void;
}

export function OpenViewMenu({ allowed, isOpen, onOpenView, onNewPanel }: Props) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  // Light dismiss: click anywhere outside closes the menu.
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  const entries = VIEW_PANES.filter((p) => allowed.includes(p.surface));

  return (
    <div ref={root} className="relative">
      <Button
        aria-label="open view"
        aria-haspopup="menu"
        aria-expanded={open}
        size="sm"
        onClick={() => setOpen((o) => !o)}
      >
        <Plus size={12} /> Open view
      </Button>
      {open && (
        <div
          role="menu"
          aria-label="open view menu"
          className="absolute right-0 z-50 mt-1 w-48 rounded-md border border-border bg-panel p-1 text-xs shadow-[var(--shadow-1)]"
        >
          <button
            type="button"
            role="menuitem"
            aria-label="open new panel"
            className="flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-fg/6"
            onClick={() => {
              setOpen(false);
              onNewPanel();
            }}
          >
            <Plus size={12} className="text-muted" /> New panel
          </button>
          {entries.length > 0 && <div className="my-1 border-t border-border" />}
          {entries.map((p) => {
            const opened = isOpen(p.kind);
            const Icon = p.icon;
            return (
          <button
            key={p.kind}
            type="button"
            role="menuitem"
            aria-label={`open ${p.title.toLowerCase()} view`}
            className={cn(
              "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left hover:bg-fg/6",
              opened && "text-muted",
            )}
                onClick={() => {
                  setOpen(false);
                  onOpenView(p.kind);
                }}
              >
                <Icon size={12} className="text-muted" />
                <span className="min-w-0 flex-1">{p.title}</span>
                {opened && <span className="text-[0.65rem] text-muted">open</span>}
              </button>
            );
          })}
        </div>
      )}
    </div>
  );
}
