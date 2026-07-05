// The compact builder toolbar (data-studio-10x scope, phase 3 stage 1) — ONE toolbar row: the inline
// title, Run, the preview toggles (freeze / table view / inspect — same affordances the split-layout
// PreviewToolbar carries, same aria names), and ONE Save split-button (primary: save to the tab;
// menu: save as a library panel — shown only for a `panel.save`-capable session; the host re-checks
// the verb server-side regardless). One responsibility: the toolbar chrome; the actions are injected.

import { useEffect, useRef, useState } from "react";
import { Check, ChevronDown, Library, Pause, Play, Search, Table2 } from "lucide-react";

import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";

interface Props {
  title: string;
  onTitle: (title: string) => void;
  canRun: boolean;
  loading: boolean;
  onRun: () => void;
  frozen: boolean;
  onToggleFreeze: () => void;
  tableView: boolean;
  onToggleTableView: () => void;
  onInspect: () => void;
  saveLabel: string;
  onSave: () => void;
  /** Save-as-library entry for the split menu — `null` when the session lacks `panel.save`. */
  onSaveAsLibrary: (() => void) | null;
}

export function BuilderToolbar(p: Props) {
  const [menu, setMenu] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menu) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setMenu(false);
    };
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [menu]);

  return (
    <div aria-label="builder toolbar" className="flex items-center gap-2">
      <Input
        aria-label="panel title"
        className="h-8 text-sm"
        placeholder="Panel title"
        value={p.title}
        onChange={(e) => p.onTitle(e.target.value)}
      />
      <Button
        aria-label="run query"
        size="sm"
        variant="outline"
        className="shrink-0"
        disabled={!p.canRun || p.frozen}
        onClick={p.onRun}
      >
        <Play size={12} /> {p.loading ? "Running…" : "Run"}
      </Button>
      <Button
        aria-label="freeze preview data"
        aria-pressed={p.frozen}
        size="icon"
        variant={p.frozen ? "default" : "ghost"}
        className="h-8 w-8 shrink-0"
        title={p.frozen ? "Unfreeze — re-fetch on edits" : "Freeze — edit without re-querying"}
        onClick={p.onToggleFreeze}
      >
        <Pause size={12} />
      </Button>
      <Button
        aria-label="toggle table view"
        aria-pressed={p.tableView}
        size="icon"
        variant={p.tableView ? "default" : "ghost"}
        className="h-8 w-8 shrink-0"
        title="Inspect the transformed frames as a table"
        onClick={p.onToggleTableView}
      >
        <Table2 size={12} />
      </Button>
      <Button
        aria-label="inspect data"
        size="icon"
        variant="ghost"
        className="h-8 w-8 shrink-0"
        title="Panel inspect — frames / JSON / resolved query"
        onClick={p.onInspect}
      >
        <Search size={12} />
      </Button>
      <div ref={root} className="relative flex shrink-0">
        <Button
          aria-label="save panel"
          size="sm"
          className={cn(p.onSaveAsLibrary && "rounded-r-none")}
          onClick={p.onSave}
        >
          <Check size={12} /> {p.saveLabel}
        </Button>
        {p.onSaveAsLibrary && (
          <>
            <Button
              aria-label="more save options"
              aria-haspopup="menu"
              aria-expanded={menu}
              size="sm"
              className="rounded-l-none border-l border-bg/30 px-1.5"
              onClick={() => setMenu((m) => !m)}
            >
              <ChevronDown size={12} />
            </Button>
            {menu && (
              <div
                role="menu"
                aria-label="save options"
                className="absolute right-0 top-full z-50 mt-1 w-48 rounded-md border border-border bg-panel p-1 text-xs shadow-[var(--shadow-1)]"
              >
                <button
                  type="button"
                  role="menuitem"
                  aria-label="save as library panel"
                  className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-fg/6"
                  onClick={() => {
                    setMenu(false);
                    p.onSaveAsLibrary?.();
                  }}
                >
                  <Library size={12} className="text-muted" /> Save as library panel
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
